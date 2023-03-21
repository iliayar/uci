use thiserror::Error;
use warp::Filter;

use super::{docker, filters};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(about)]
struct Args {
    /// TCP port to run on
    #[arg(short, long, default_value_t = 3001)]
    port: u16,
}

pub struct App {
    docker: docker::Docker,
    port: u16,
}

#[derive(Error, Debug)]
pub enum WorkerError {
    #[error("Failed to initilize docker connection")]
    DockerError(#[from] bollard::errors::Error),
}

impl App {
    pub async fn init() -> Result<App, WorkerError> {
	let args = Args::parse();

        let app = App {
            docker: docker::Docker::init()?,
	    port: args.port,
        };

        pretty_env_logger::init();

        Ok(app)
    }

    pub async fn run(self) {
        let api = filters::runner(self.docker);
        let routes = api.with(warp::log("worker"));

        warp::serve(routes).run(([127, 0, 0, 1], self.port)).await;
    }
}
