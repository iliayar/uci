use super::docker::Docker;

#[derive(Clone)]
pub struct Context {
    docker: Docker,
}

impl Context {
    pub fn new(docker: Docker) -> Context {
	Context {
	    docker,
	}
    }

    pub fn docker(&self) -> &Docker {
	&self.docker
    }
}
