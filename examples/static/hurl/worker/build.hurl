POST http://localhost:3001/run
{
  "steps": [
    {
      "BuildImage": {
          "source": {
	    "Archive": {
	      "tar_path": "/home/iliayar/Envs/docker/ubuntu-with-nc.tar.gz",
	      "tag": "microci-test-1"
	    }
	  }
       }
    }
  ]
}
