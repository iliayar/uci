import pytest
from models import Pipeline


def test_empty_projects_list(backend):
    """Test that the backend starts with an empty projects list"""
    response = backend.call_api("/projects/list")
    assert response.status_code == 200
    # For empty config, the projects list should be empty
    assert response.json() == {"projects": []}


@pytest.mark.project(id="test-project")
def test_project_configuration(backend):
    """Test that a project configuration is loaded correctly"""
    response = backend.call_api("/projects/list")
    assert response.status_code == 200
    data = response.json()
    
    # Verify the project exists
    assert len(data["projects"]) == 1
    assert data["projects"][0]["id"] == "test-project"


@pytest.mark.project(
    id="multi-config",
    pipelines={
        "test-pipeline": Pipeline(
            jobs={
                "echo-job": {
                    "do": "run",
                    "steps": [
                        {
                            "name": "echo",
                            "run": "echo 'Hello, World!'"
                        }
                    ]
                }
            }
        )
    }
)
def test_reload_config(backend):
    """Test that configuration can be reloaded at runtime"""
    # Verify initial configuration
    response = backend.call_api("/projects/list")
    assert response.status_code == 200
    assert len(response.json()["projects"]) == 1
    
    # Get pipelines for the project
    response = backend.call_api("/projects/pipelines/list", params={"project_id": "multi-config"})
    assert response.status_code == 200
    data = response.json()
    assert len(data["pipelines"]) == 1
    assert data["pipelines"][0]["id"] == "test-pipeline"
    
    # Now test the reload functionality
    backend.reload_config()
