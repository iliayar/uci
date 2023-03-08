pub struct App {
    docker: handlers::docker::Docker,
}

use warp::Filter;
use thiserror::Error;

use super::{filters, handlers};

#[derive(Error, Debug)]
pub enum RunnerError {
    #[error("Failed to initilize docker connection")]
    DockerError(#[from] handlers::docker::DockerError)
}

impl App {
    pub async fn init() -> Result<App, RunnerError> {
        let app = App {
            docker: handlers::docker::Docker::init()?,
        };

        pretty_env_logger::init();

        Ok(app)
    }

    pub async fn run(self) {
        let api = filters::runner(self.docker);
        let routes = api.with(warp::log("worker"));

        warp::serve(routes).run(([127, 0, 0, 1], 8080)).await;
    }
}
