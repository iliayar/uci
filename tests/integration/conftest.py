import os
import subprocess
import time
import pytest
import requests
import yaml
import tempfile
import shutil
import pathlib

import models

class BackendContainer:
    def __init__(self, config_dir):
        self.config_dir = config_dir
        self.api_url = None
        self.container_id = None

    def start(self):
        """Start the backend container"""
        # Start the container without --rm so we can get logs if it fails
        cmd = [
            "docker", "run", "-d",
            "-v", f"{self.config_dir}:/app/config:ro",
            "-v", "/var/run/docker.sock:/var/run/docker.sock",
            "uci-backend:test"
        ]
        result = subprocess.run(cmd, check=True, capture_output=True, text=True)
        self.container_id = result.stdout.strip()
        time.sleep(1)
        
        # Check if the container is actually running
        check_cmd = ["docker", "ps", "--filter", f"id={self.container_id}", "--format", "{{.Status}}"]
        status = subprocess.run(check_cmd, capture_output=True, text=True, check=True)
        if "Up" not in status.stdout:
            self._cleanup()
            raise RuntimeError(f"Container {self.container_id} failed to start")
            
        self._set_container_ip()
        self._wait_for_startup()

    def _cleanup(self):
        if self.container_id:
            logs_cmd = ["docker", "logs", self.container_id]
            logs = subprocess.run(logs_cmd, capture_output=True, text=True)
            print(f"Container logs:\n{logs.stdout}\n{logs.stderr}")

            subprocess.run(["docker", "rm", self.container_id], capture_output=True)
    
    def _set_container_ip(self):
        """Get container IP address"""
        # Get the network settings specifically for the uci-test-network
        assert self.container_id is not None
        cmd = ["docker", "inspect", "-f", "{{range.NetworkSettings.Networks}}{{.IPAddress}}{{end}}", self.container_id]
        result = subprocess.run(cmd, check=True, capture_output=True, text=True)
        ip = result.stdout.strip()
        
        # If the specific network search fails, try all networks
        if not ip:
            raise RuntimeError(f"Could not get IP of container {self.container_id}")
        self.api_url = f"http://{ip}:3002"

    def stop(self):
        """Stop the backend container"""
        assert self.container_id is not None
        subprocess.run(["docker", "kill", self.container_id], check=True, capture_output=True)
        self._cleanup()

    def reload_config(self):
        """Trigger config reload in the backend"""
        response = requests.post(f"{self.api_url}/reload")
        response.raise_for_status()
        return response.json()

    def call_api(self, endpoint, method="get", data=None, params=None):
        """Make an API call to the backend"""
        url = f"{self.api_url}{endpoint}"
        response = getattr(requests, method.lower())(url, json=data, params=params)
        return response

    def _wait_for_startup(self, timeout=10, interval=0.5):
        """Wait for the backend to start up"""
        start_time = time.time()
        while time.time() - start_time < timeout:
            try:
                response = requests.get(f"{self.api_url}/ping")
                if response.status_code == 200:
                    return
            except requests.RequestException as e:
                print(f"Waiting for backend to start: {e}")
            time.sleep(interval)
        
        raise TimeoutError(f"Backend service did not start within the expected timeout. API URL: {self.api_url}")


@pytest.fixture(scope="session")
def docker_image():
    """Build the Docker image for testing"""
    # Build the image with a specific tag for testing, silently
    subprocess.run(
        ["docker", "build", "-q", "-t", "uci-backend:test", "-f", "docker/backend/Dockerfile", "."],
        cwd=str(pathlib.Path(__file__).parents[2]),  # Root of the project
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL
    )
    yield "uci-backend:test"


@pytest.fixture
def config_dir(request):
    """Create a temporary directory for config files with content from the config mark"""
    temp_dir = tempfile.mkdtemp()
    
    # Check if test is marked with custom config
    marker = request.node.get_closest_marker("config")

    config = models.Config(
        *([] if not marker else marker.args),
        **({} if not marker else marker.kwargs)
    )

    for marker in request.node.iter_markers():
        if marker.name != 'project':
            continue
        
        config.projects.append(models.Project(*marker.args, **marker.kwargs))

    default_config = models.generate_yaml_config(config)
    with open(os.path.join(temp_dir, "config.yaml"), "w") as f:
        yaml.dump(default_config, f)
    
    yield temp_dir
    shutil.rmtree(temp_dir)


@pytest.fixture
def backend(docker_image, config_dir):
    """Fixture providing a running backend container configured from the config mark"""
    # Create the backend instance
    backend = BackendContainer(config_dir)
    
    # Start the container
    backend.start()
    
    yield backend
    
    # Cleanup
    backend.stop()
