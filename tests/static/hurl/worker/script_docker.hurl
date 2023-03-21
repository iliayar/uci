POST http://localhost:3001/run
{
  "steps": [
    {
      "RunShell": {
        "script": "set -e\necho Hello\n\nid\necho This is error >&2\nsleep 2\necho This is not error",
	"docker_image": "ubuntu"
       }
    }
  ]
}
