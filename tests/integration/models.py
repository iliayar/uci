import dataclasses
import typing as tp
from datetime import datetime

@dataclasses.dataclass
class Script:
    """A step in a job"""
    script: str
    type: str = "script"

@dataclasses.dataclass
class Job:
    """A job in a pipeline"""
    do: tp.List[Script] = dataclasses.field(default_factory=list)
    needs: tp.Optional[tp.List[str]] = None
    stage: tp.Optional[str] = None

@dataclasses.dataclass
class Pipeline:
    """A pipeline definition"""
    jobs: tp.Dict[str, tp.Union[Job, tp.Dict[str, tp.Any]]] = dataclasses.field(default_factory=dict)
    links: tp.Optional[tp.Dict[str, tp.List[str]]] = None
    stages: tp.Optional[tp.List[str]] = None
    integrations: tp.Optional[tp.List[tp.Dict[str, tp.Any]]] = None

@dataclasses.dataclass
class Action:
    """An action definition"""
    on: tp.Optional[str] = None
    run_pipelines: tp.Optional[tp.List[str]] = None
    services: tp.Optional[tp.List[str]] = None
    repo_id: tp.Optional[str] = None
    changes: tp.Optional[tp.List[str]] = None
    exclude_changes: tp.Optional[tp.List[str]] = None
    exclude_commits: tp.Optional[tp.List[str]] = None
    params: tp.Optional[tp.Dict[str, tp.Any]] = None

@dataclasses.dataclass
class Project:
    id: str
    pipelines: tp.Dict[str, Pipeline] = dataclasses.field(default_factory=dict)
    actions: tp.Dict[str, tp.List[Action]] = dataclasses.field(default_factory=dict)
    params: tp.Optional[tp.Dict[str, tp.Any]] = None
    docker: tp.Optional[tp.Dict[str, tp.Any]] = None
    bind: tp.Optional[tp.Dict[str, tp.Any]] = None
    caddy: tp.Optional[tp.Dict[str, tp.Any]] = None

@dataclasses.dataclass
class Config:
    projects: tp.List[Project] = dataclasses.field(default_factory=list)

@dataclasses.dataclass
class WebSocketMessage:
    """Base class for WebSocket messages"""
    type: str

@dataclasses.dataclass
class LogMessage(WebSocketMessage):
    """Log message from a run"""
    pipeline: str
    job_id: str
    text: str
    timestamp: int
    message_type: str = "log"
    
    @classmethod
    def from_dict(cls, data):
        if isinstance(data, dict) and "Log" in data:
            log_data = data["Log"]
            return cls(
                type="log",
                pipeline=log_data.get("pipeline", ""),
                job_id=log_data.get("job_id", ""),
                text=log_data.get("text", ""),
                timestamp=log_data.get("timestamp", 0)
            )
        return None

@dataclasses.dataclass
class StatusMessage(WebSocketMessage):
    """Status update message from a run"""
    status: tp.Dict[str, tp.Any]
    
    @classmethod
    def from_dict(cls, data):
        if isinstance(data, dict) and "Status" in data:
            return cls(
                type="status",
                status=data["Status"]
            )
        return None

@dataclasses.dataclass
class DoneMessage(WebSocketMessage):
    """Message indicating the WebSocket stream is done"""
    
    @classmethod
    def from_dict(cls, data):
        if isinstance(data, dict) and "Done" in data:
            return cls(type="done")
        return None

def generate_yaml_config(config_obj: Config) -> tp.Dict[str, tp.Any]:
    """Generate YAML configuration dictionary from Config object"""
    projects_dict = {}
    
    for project in config_obj.projects:
        # Convert dataclass to dict
        project_dict = dataclasses.asdict(project)
        project_id = project_dict.pop('id')  # Remove ID from dict as it's used as key
        
        # Add the project to the projects dict with permissions
        projects_dict[project_id] = {
            "config": project_dict,  # Direct embedding of project config
            "tokens": [{
                    "permissions": ["read", "write", "execute"]
            }]
        }
    
    # Create main config structure
    config_dict = {
        "projects_store": {
            "type": "static",
            "projects": projects_dict
        },
        "tokens": [{
            "permissions": ["read", "write", "execute"]
        }]
    }
    
    return config_dict
