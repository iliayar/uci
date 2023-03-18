POST http://localhost:3001/run
{
  "steps": [
    {
      "BuildImage": {
          "source": {
	    "Pull": {
	      "image": "ubuntu"
	    }
	  }
       }
    }
  ]
}
