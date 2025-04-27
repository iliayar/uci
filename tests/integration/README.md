# uCI Integration Tests

This directory contains integration tests for the uCI backend using pytest and Docker.

## Prerequisites

- Docker installed and running
- Python 3.7+ with pip
- pytest and other requirements installed

## Setup

Install the required Python packages:

```bash
pip install -r requirements.txt
```

## Running Tests

From the project root directory:

```bash
# Run all integration tests
pytest tests/integration

# Run a specific test file
pytest tests/integration/test_projects.py

# Run a specific test
pytest tests/integration/test_projects.py::test_empty_projects_list
```

## Writing Tests

### Custom Configuration

You can provide custom configuration using the `@pytest.mark.config` marker:

```python
@pytest.mark.config({
    "config.yaml": {
        "projects_store": {
            "type": "static",
            "projects": {
                "test-project": {
                    "config": "${load(./projects/test-project/project.yaml)}"
                }
            }
        }
    },
    "projects/test-project/project.yaml": {
        "repos": {},
        "actions": [],
        "pipelines": []
    }
})
def test_with_custom_config(backend):
    # Your test code here
    pass
```

### Using the Backend Fixture

The `backend` fixture provides a running uCI backend with the specified configuration:

```python
def test_example(backend):
    # Make API calls to the backend
    response = backend.call_api("/api/projects")
    assert response.status_code == 200
    
    # Reload configuration if needed
    backend.reload_config()
```

## Test Isolation

Each test gets its own isolated Docker container with a fresh configuration. The container is automatically cleaned up after the test completes, regardless of whether the test passes or fails.