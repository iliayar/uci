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

class ProjectClient:
    """Client for interacting with a specific project in the backend"""
    
    def __init__(self, backend_client, project_id):
        self.backend = backend_client
        self.id = project_id
        
    def list_pipelines(self):
        """List all pipelines in the project"""
        response = self.backend.call_api(
            "/projects/pipelines/list", 
            params={"project_id": self.id}
        )
        response.raise_for_status()
        return response.json()["pipelines"]
    
    def get_pipeline(self, pipeline_id):
        """Get a specific pipeline by ID"""
        pipelines = self.list_pipelines()
        for pipeline in pipelines:
            if pipeline["id"] == pipeline_id:
                return pipeline
        return None
        
    def list_actions(self):
        """List all actions in the project"""
        response = self.backend.call_api(
            "/projects/actions/list", 
            params={"project_id": self.id}
        )
        response.raise_for_status()
        return response.json()["actions"]
    
    def get_action(self, action_id):
        """Get a specific action by ID"""
        actions = self.list_actions()
        for action in actions:
            if action["id"] == action_id:
                return action
        return None
    
    def call_action(self, action_id, dry_run=False, params=None):
        """Call an action in this project
        
        Args:
            action_id: ID of the action to call
            dry_run: If True, simulate the action without actually running it
            params: Optional parameters to pass to the action
        
        Returns:
            Response data containing run_id
        """
        data = {
            "project_id": self.id,
            "trigger_id": action_id,
            "dry_run": dry_run
        }
        
        if params:
            data["params"] = params
            
        response = self.backend.call_api(
            "/call",
            method="post",
            data=data
        )
        response.raise_for_status()
        return response.json()
    
    def get_run(self, pipeline_id, run_id):
        """Get details of a specific run"""
        runs = self.list_runs(pipeline_id)
        print(runs)
        for run in runs:
            if run["run_id"] == run_id:
                return run
        return None
    
    def list_runs(self, pipeline_id=None):
        """List all runs for this project, optionally filtered by pipeline"""
        params = {"project_id": self.id}
        if pipeline_id:
            params["pipeline_id"] = pipeline_id
            
        response = self.backend.call_api("/runs/list", params=params)
        response.raise_for_status()
        return response.json()["runs"]
    
    def get_run_logs(self, pipeline_id, run, timeout=2):
        """Get logs for a specific run
        
        This method connects to the WebSocket endpoint to retrieve the full logs
        for a run. It handles the initial API call to get the WebSocket connection 
        ID and then collects all the logs from the WebSocket stream.
        
        Args:
            pipeline_id: ID of the pipeline
            run: ID of the run
            timeout: Maximum time to wait for logs in seconds
            
        Returns:
            List of LogMessage, StatusMessage, and DoneMessage objects
        """
        import websocket
        import json
        import threading
        import time
        from models import LogMessage, StatusMessage, DoneMessage
        
        # First get the WebSocket ID
        response = self.backend.call_api(
            "/runs/logs", 
            params={
                "project": self.id,
                "pipeline": pipeline_id,
                "run": run,
            }
        )
        response.raise_for_status()
        
        # Try multiple possible field names
        run_id = response.json()["run_id"]
        
        # Connect to the WebSocket to get the logs stream
        messages = []
        received_all = threading.Event()
        
        def on_message(ws, message):
            data = json.loads(message)
            print(f"WebSocket message: {data}")
            
            # Try to parse based on the message structure
            log_msg = LogMessage.from_dict(data)
            if log_msg:
                messages.append(log_msg)
                return
                
            status_msg = StatusMessage.from_dict(data)
            if status_msg:
                messages.append(status_msg)
                return
                
            done_msg = DoneMessage.from_dict(data)
            if done_msg:
                messages.append(done_msg)
                received_all.set()
                return
                
            # If we reach here, we couldn't handle the message
            # raise RuntimeError(f"Unrecognized message format: {data}")
            pass
                
        def on_error(ws, error):
            pass
            
        def on_close(ws, close_status_code, close_msg):
            if not received_all.is_set():
                # raise RuntimeError("WebSocket closed unexpectedly")
                pass
            received_all.set()
        
        def on_open(ws):
            pass
        
        # Create WebSocket connection using the host directly
        ws_url = f"ws://{self.backend.api_host}/ws/{run_id}"
        
        ws = websocket.WebSocketApp(
            ws_url,
            on_open=on_open,
            on_message=on_message,
            on_error=on_error,
            on_close=on_close
        )
        
        # Start WebSocket in a separate thread
        wst = threading.Thread(target=ws.run_forever)
        wst.daemon = True
        wst.start()
        
        # Wait for logs to be received or timeout
        start_time = time.time()
        while not received_all.is_set() and time.time() - start_time < timeout:
            time.sleep(0.1)
            
        # If we hit the timeout without receiving "done" message
        if not received_all.is_set():
            raise RuntimeError(f"Timed out waiting for logs after {timeout} seconds")
            
        # Close the websocket connection
        ws.close()
        return messages
        
    def wait_for_run(self, pipeline_id, run_id, timeout=2, interval=0.5):
        """Wait for a run to appear in the runs list
        
        In test environments, there might be a delay before a run is visible.
        This method polls until the run appears or the timeout is reached.
        """
        import time
        start_time = time.time()
        while time.time() - start_time < timeout:
            run = self.get_run(pipeline_id, run_id)
            print(f"Run: {run}")
            if run and isinstance(run["status"], dict) and "Finished" in run["status"]:
                return run
            time.sleep(interval)
        
        return None


class BackendBase:
    """Base class for backend implementations"""
    
    def __init__(self, config_dir):
        self.config_dir = config_dir
        self.api_url = None
        self.api_host = None
        self._projects_cache = {}
        self._pipeline_cache = {}
    
    def call_api(self, endpoint, method="get", data=None, params=None):
        """Make a raw API call to the backend"""
        url = f"{self.api_url}{endpoint}"
        response = getattr(requests, method.lower())(url, json=data, params=params)
        return response
    
    def reload_config(self):
        """Trigger config reload in the backend"""
        response = self.call_api("/reload", method="post")
        response.raise_for_status()
        # Clear all caches after reloading
        self._projects_cache = {}
        self._pipeline_cache = {}
        return response.json()
        
    def stop(self):
        """Stop the backend - to be implemented by subclasses"""
        raise NotImplementedError("Subclasses must implement stop method")
        
    def _wait_for_startup(self, timeout=10, interval=0.5):
        """Wait for the backend to start up"""
        start_time = time.time()
        while time.time() - start_time < timeout:
            try:
                response = self.call_api("/ping")
                if response.status_code == 200:
                    return
            except requests.RequestException as e:
                print(f"Waiting for backend to start: {e}")
            time.sleep(interval)
        
        raise TimeoutError(f"Backend service did not start within the expected timeout. API URL: {self.api_url}")
    
    def list_projects(self):
        """List all projects in the backend"""
        response = self.call_api("/projects/list")
        response.raise_for_status()
        
        # Get projects data from response
        projects_data = response.json()["projects"]
        
        # Update projects cache with ProjectClient instances
        for project_data in projects_data:
            project_id = project_data["id"]
            if project_id not in self._projects_cache:
                # Create a new ProjectClient instance and store the data in it
                client = ProjectClient(self, project_id)
                client._metadata = project_data  # Store metadata in the client
                self._projects_cache[project_id] = client
            else:
                # Update existing client's metadata
                self._projects_cache[project_id]._metadata = project_data
            
        return projects_data
    
    def get_project(self, project_id):
        """Get a project's metadata by ID"""
        if project_id not in self._projects_cache:
            # Ensure the project exists by refreshing the list
            self.list_projects()
            if project_id not in self._projects_cache:
                return None
        
        # Return the metadata stored in the ProjectClient
        return getattr(self._projects_cache[project_id], '_metadata', None)
    
    def project(self, project_id):
        """Get a client for a specific project"""
        if project_id not in self._projects_cache:
            # Ensure the project exists by refreshing the list
            self.list_projects()
            if project_id not in self._projects_cache:
                raise ValueError(f"Project '{project_id}' not found")
        
        # Return the cached ProjectClient
        return self._projects_cache[project_id]
    
    def __getattr__(self, name):
        """Allow accessing projects as attributes, e.g., backend.project_name"""
        # Convert attribute name to project ID format (replace underscores with hyphens)
        project_id = name.replace("_", "-")
        
        try:
            return self.project(project_id)
        except ValueError:
            # Try the original name if the converted name didn't work
            if project_id != name:
                try:
                    return self.project(name)
                except ValueError:
                    pass
            
            raise AttributeError(f"'{self.__class__.__name__}' has no attribute '{name}'")


def find_free_port():
    """Find a free port on localhost"""
    import socket
    from contextlib import closing
    
    with closing(socket.socket(socket.AF_INET, socket.SOCK_STREAM)) as s:
        s.bind(('', 0))
        s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        return s.getsockname()[1]


class BackendProcess(BackendBase):
    """Process-based backend implementation using local binary"""
    
    def __init__(self, config_dir):
        super().__init__(config_dir)
        self.process = None
        self.binary_path = None
        self.port = None
        self.find_binary()
    
    def find_binary(self):
        """Find the UCI daemon binary in the target directory"""
        project_root = pathlib.Path(__file__).parents[2]  # Root of the project
        possible_paths = [
            project_root / "target" / "debug" / "ucid",
            project_root / "target" / "release" / "ucid",
        ]
        
        for path in possible_paths:
            if path.exists() and os.access(path, os.X_OK):
                self.binary_path = path
                return
        
        raise FileNotFoundError("Could not find ucid binary in target directory. Please build it with 'cargo build'")
    
    def _cleanup(self):
        """Clean up the process resources and display output"""
        if self.process:
            try:
                # Try to terminate gracefully first
                if self.process.poll() is None:  # Only terminate if still running
                    self.process.terminate()
                    try:
                        self.process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        # If it doesn't exit in 5 seconds, force kill it
                        self.process.kill()
                        self.process.wait()
            except ProcessLookupError:
                # Process is already gone
                pass
            
            # Capture and display output
            stdout, stderr = self.process.communicate()
            print(f"Process output:\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}")
            
            self.process = None
    
    def start(self):
        """Start the backend process"""
        # Find a free port
        self.port = find_free_port()
        
        # Set up the environment
        env = os.environ.copy()
        
        # Start the process with correct command line arguments
        config_path = os.path.join(self.config_dir, "config.yaml")
        cmd = [
            str(self.binary_path),
            "--port", str(self.port),
            "--config", config_path,
        ]
        
        self.process = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
            text=True
        )
        
        # Set up the API URL
        self.api_host = f"localhost:{self.port}"
        self.api_url = f"http://{self.api_host}"
        
        # Wait for the process to start
        try:
            self._wait_for_startup()
        except TimeoutError:
            # Process didn't start properly, clean up before raising the exception
            self._cleanup()
            raise
    
    def stop(self):
        """Stop the backend process"""
        self._cleanup()


class BackendContainer(BackendBase):
    """Docker-based backend implementation"""
    
    def __init__(self, config_dir):
        super().__init__(config_dir)
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
            print(f"Process output:\nSTDOUT:\n{logs.stdout}\nSTDERR:\n{logs.stderr}")

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
        
        # Store just the host and port, without the scheme
        self.api_host = f"{ip}:3002"
        self.api_url = f"http://{self.api_host}"

    def stop(self):
        """Stop the backend container"""
        assert self.container_id is not None
        subprocess.run(["docker", "kill", self.container_id], check=True, capture_output=True)
        self._cleanup()

    def _wait_for_startup(self, timeout=10, interval=0.5):
        """Wait for the backend to start up"""
        start_time = time.time()
        while time.time() - start_time < timeout:
            try:
                response = self.call_api("/ping")
                if response.status_code == 200:
                    return
            except requests.RequestException as e:
                print(f"Waiting for backend to start: {e}")
            time.sleep(interval)
        
        raise TimeoutError(f"Backend service did not start within the expected timeout. API URL: {self.api_url}")
    
    # High-level API methods
    
    def list_projects(self):
        """List all projects in the backend"""
        response = self.call_api("/projects/list")
        response.raise_for_status()
        
        # Get projects data from response
        projects_data = response.json()["projects"]
        
        # Update projects cache with ProjectClient instances
        for project_data in projects_data:
            project_id = project_data["id"]
            if project_id not in self._projects_cache:
                # Create a new ProjectClient instance and store the data in it
                client = ProjectClient(self, project_id)
                client._metadata = project_data  # Store metadata in the client
                self._projects_cache[project_id] = client
            else:
                # Update existing client's metadata
                self._projects_cache[project_id]._metadata = project_data
            
        return projects_data
    
    def get_project(self, project_id):
        """Get a project's metadata by ID"""
        if project_id not in self._projects_cache:
            # Ensure the project exists by refreshing the list
            self.list_projects()
            if project_id not in self._projects_cache:
                return None
        
        # Return the metadata stored in the ProjectClient
        return getattr(self._projects_cache[project_id], '_metadata', None)
    
    def project(self, project_id):
        """Get a client for a specific project"""
        if project_id not in self._projects_cache:
            # Ensure the project exists by refreshing the list
            self.list_projects()
            if project_id not in self._projects_cache:
                raise ValueError(f"Project '{project_id}' not found")
        
        # Return the cached ProjectClient
        return self._projects_cache[project_id]
    
    
    def __getattr__(self, name):
        """Allow accessing projects as attributes, e.g., backend.project_name"""
        # Convert attribute name to project ID format (replace underscores with hyphens)
        project_id = name.replace("_", "-")
        
        try:
            return self.project(project_id)
        except ValueError:
            # Try the original name if the converted name didn't work
            if project_id != name:
                try:
                    return self.project(name)
                except ValueError:
                    pass
            
            raise AttributeError(f"'BackendContainer' has no attribute '{name}'")


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
def backend(request, config_dir):
    """Fixture providing a running backend instance configured from the config mark
    
    By default, this fixture uses the BackendProcess implementation which runs the 
    local binary from the target directory. If a test is marked with @pytest.mark.docker,
    it will use the Docker-based implementation instead.
    """
    # Check if the test is marked to use Docker
    docker_marker = request.node.get_closest_marker("docker")
    
    if docker_marker:
        # Make sure the Docker image is built
        subprocess.run(
            ["docker", "build", "-q", "-t", "uci-backend:test", "-f", "docker/backend/Dockerfile", "."],
            cwd=str(pathlib.Path(__file__).parents[2]),  # Root of the project
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL
        )
        backend = BackendContainer(config_dir)
    else:
        # Use the local binary implementation by default
        backend = BackendProcess(config_dir)
    
    # Start the backend
    backend.start()
    
    yield backend
    
    # Cleanup
    backend.stop()
