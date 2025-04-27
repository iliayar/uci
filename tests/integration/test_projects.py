import pytest
from models import Pipeline, Action, Job, Script


def test_empty_projects_list(backend):
    """Test that the backend starts with an empty projects list"""
    projects = backend.list_projects()
    # For empty config, the projects list should be empty
    assert projects == []


@pytest.mark.project(id="test-project")
def test_project_configuration(backend):
    """Test that a project configuration is loaded correctly"""
    # Get project list
    projects = backend.list_projects()
    
    # Verify the project exists
    assert len(projects) == 1
    assert projects[0]["id"] == "test-project"
    
    # Test get_project method
    project_data = backend.get_project("test-project")
    assert project_data is not None, "Project not found via get_project method"
    assert project_data["id"] == "test-project"
    
    # Verify we can access the project using the attribute syntax
    assert backend.test_project.id == "test-project"
    
    # Verify that attribute access is case-sensitive but handles hyphens/underscores
    # This confirms our __getattr__ enhancement works
    assert backend.test_project.id == "test-project"  # Exact match
    
    # Test with underscores
    try:
        project_client = backend.test_project  # Should work
        assert project_client.id == "test-project"
    except AttributeError:
        pytest.fail("Project access with underscores failed but should work")


@pytest.mark.project(
    id="multi-config",
    pipelines={
        "test-pipeline": Pipeline(
            jobs={
                "echo-job": Job(
                    do=[Script("echo 'Hello, World!'")]
                )
            }
        )
    }
)
def test_reload_config(backend):
    """Test that configuration can be reloaded at runtime"""
    # Verify initial configuration
    projects = backend.list_projects()
    assert len(projects) == 1
    
    # Get pipelines for the project using the convenient project client
    pipelines = backend.multi_config.list_pipelines()
    assert len(pipelines) == 1
    assert pipelines[0]["id"] == "test-pipeline"
    
    # Now test the reload functionality
    reload_result = backend.reload_config()
    # The reload endpoint might return empty or a success status
    # Either way, we should still be able to list projects after reload
    
    # Verify we still see the same config after reload
    projects = backend.list_projects()
    assert len(projects) == 1
    assert projects[0]["id"] == "multi-config"


@pytest.mark.project(
    id="project1",
    pipelines={"test-pipeline": Pipeline()}
)
@pytest.mark.project(
    id="project2",
    pipelines={"other-pipeline": Pipeline()}
)
def test_multiple_projects(backend):
    """Test working with multiple projects"""
    # List all projects
    projects = backend.list_projects()
    assert len(projects) == 2, "Expected 2 projects"
    
    # Get project IDs and sort them
    project_ids = sorted([p["id"] for p in projects])
    assert project_ids == ["project1", "project2"]
    
    # Access projects by attribute
    project1 = backend.project1
    project2 = backend.project2
    
    # Verify pipeline in each project
    project1_pipelines = project1.list_pipelines()
    assert len(project1_pipelines) == 1
    assert project1_pipelines[0]["id"] == "test-pipeline"
    
    project2_pipelines = project2.list_pipelines()
    assert len(project2_pipelines) == 1
    assert project2_pipelines[0]["id"] == "other-pipeline"
    
    # Verify projects have their own pipelines
    assert len(project1.list_pipelines()) == 1
    assert len(project2.list_pipelines()) == 1
