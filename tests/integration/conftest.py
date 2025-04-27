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
    
    def get_run_logs(self, pipeline_id, run_id):
        """Get logs for a specific run
        
        This method connects to the WebSocket endpoint to retrieve the full logs
        for a run. It handles the initial API call to get the WebSocket connection 
        ID and then collects all the logs from the WebSocket stream.
        
        Args:
            pipeline_id: ID of the pipeline
            run_id: ID of the run
            
        Returns:
            List of log entries from the run
        """
        import websocket
        import json
        import threading
        from queue import Queue
        
        # First get the WebSocket ID
        response = self.backend.call_api(
            "/runs/logs", 
            params={
                "project": self.id,
                "pipeline": pipeline_id,
                "run": run_id
            }
        )
        response.raise_for_status()
        ws_id = response.json().get("run_id")
        
        if not ws_id:
            return []
        
        # Connect to the WebSocket to get the logs stream
        logs = []
        received_all = threading.Event()
        message_queue = Queue()
        
        def on_message(ws, message):
            try:
                data = json.loads(message)
                if data.get("type") == "log":
                    logs.append(data)
                elif data.get("type") == "done":
                    received_all.set()
            except json.JSONDecodeError:
                # Handle invalid JSON if needed
                pass
                
        def on_error(ws, error):
            message_queue.put(f"WebSocket error: {error}")
            received_all.set()
            
        def on_close(ws, close_status_code, close_msg):
            received_all.set()
            
        # Create WebSocket connection
        ws_url = f"ws://{self.backend.api_url.split('://')[1]}/ws/{ws_id}"
        ws = websocket.WebSocketApp(
            ws_url,
            on_message=on_message,
            on_error=on_error,
            on_close=on_close
        )
        
        # Start WebSocket in a separate thread
        wst = threading.Thread(target=ws.run_forever)
        wst.daemon = True
        wst.start()
        
        # Wait for logs to be received or timeout
        received_all.wait(timeout=30)  # 30 second timeout
        ws.close()
        
        # Process any errors
        if not message_queue.empty():
            error_message = message_queue.get()
            raise RuntimeError(f"Error getting logs: {error_message}")
        
        return logs
        
    def wait_for_run(self, run_id, timeout=30, interval=0.5):
        """Wait for a run to appear in the runs list
        
        In test environments, there might be a delay before a run is visible.
        This method polls until the run appears or the timeout is reached.
        
        Args:
            run_id: The ID of the run to wait for
            timeout: Maximum seconds to wait
            interval: Polling interval in seconds
            
        Returns:
            Run data if found, None if timed out
        """
        import time
        start_time = time.time()
        while time.time() - start_time < timeout:
            # List all runs in the project
            all_runs = []
            for pipeline in self.list_pipelines():
                pipeline_runs = self.list_runs(pipeline["id"])
                all_runs.extend(pipeline_runs)
            
            # Check if our run is in the list
            for run in all_runs:
                if run["run_id"] == run_id:
                    return run
            
            time.sleep(interval)
        
        return None


class BackendContainer:
    def __init__(self, config_dir):
        self.config_dir = config_dir
        self.api_url = None
        self.container_id = None
        self._projects_cache = {}
        self._pipeline_cache = {}

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
        # Clear all caches after reloading
        self._projects_cache = {}
        self._pipeline_cache = {}
        return response.json()

    def call_api(self, endpoint, method="get", data=None, params=None):
        """Make a raw API call to the backend"""
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
