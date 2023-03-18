use thiserror::Error;
use warp::Filter;

use super::{docker, filters};

pub struct App {
    docker: docker::Docker,
}

#[derive(Error, Debug)]
pub enum WorkerError {
    #[error("Failed to initilize docker connection")]
    DockerError(#[from] bollard::errors::Error),
}

impl App {
    pub async fn init() -> Result<App, WorkerError> {
        let app = App {
            docker: docker::Docker::init()?,
        };

        pretty_env_logger::init();

        Ok(app)
    }

    pub async fn run(self) {
        let api = filters::runner(self.docker);
        let routes = api.with(warp::log("worker"));

        warp::serve(routes).run(([127, 0, 0, 1], 3001)).await;
    }
}
