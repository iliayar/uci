import dataclasses
import typing as tp

@dataclasses.dataclass
class Step:
    """A step in a job"""
    name: str
    run: str
    env: tp.Optional[tp.Dict[str, str]] = None
    shell: tp.Optional[str] = None
    working_dir: tp.Optional[str] = None
    continue_on_error: bool = False
    timeout: tp.Optional[int] = None

@dataclasses.dataclass
class Job:
    """A job in a pipeline"""
    do: str = "run"  # "run" or "deploy"
    steps: tp.List[tp.Union[Step, tp.Dict[str, tp.Any]]] = dataclasses.field(default_factory=list)
    environment: tp.Optional[tp.Dict[str, str]] = None
    needs: tp.Optional[tp.List[str]] = None
    working_dir: tp.Optional[str] = None
    stage: tp.Optional[str] = None
    timeout: tp.Optional[int] = None
    retry: tp.Optional[int] = None
    docker: tp.Optional[tp.Dict[str, tp.Any]] = None

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
