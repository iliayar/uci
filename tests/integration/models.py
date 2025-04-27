import dataclasses
import typing as tp

@dataclasses.dataclass
class Pipeline:
    jobs: tp.Dict[str, tp.Dict[str, tp.Any]] = dataclasses.field(default_factory=dict)
    links: tp.Optional[tp.Dict[str, tp.List[str]]] = None
    stages: tp.Optional[tp.List[str]] = None
    integrations: tp.Optional[tp.List[tp.Dict[str, tp.Any]]] = None

@dataclasses.dataclass
class Action:
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
