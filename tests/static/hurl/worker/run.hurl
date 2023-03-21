POST http://localhost:3001/run
{
  "steps": [
    {
      "RunContainer": {
        "name": "alpine-test-1",
        "image": "alpine"
       }
    }
  ]
}
