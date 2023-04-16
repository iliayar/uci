use std::sync::Arc;

use common::state::State;
use thiserror::Error;
use warp::Filter;

use super::filters;
use worker_lib::docker;

use clap::Parser;

use log::*;

#[derive(Parser, Debug)]
#[command(about)]
struct Args {
    /// TCP port to run on
    #[arg(short, long, default_value_t = 3001)]
    port: u16,
}

pub struct App {
    port: u16,
}

#[derive(Error, Debug)]
pub enum WorkerError {
    #[error("Failed to initilize docker connection: {0}")]
    DockerError(#[from] docker::DockerError),
}

impl App {
    pub async fn init() -> Result<App, WorkerError> {
        let args = Args::parse();

        let docker = docker::Docker::init()?;
        let app = App {
            port: args.port,
        };

        pretty_env_logger::init();

        Ok(app)
    }

    pub async fn run(self) {
	if let Err(err) = self.run_impl().await {
	    error!("Failed to run app: {}", err);
	}
    }

    pub async fn run_impl(self) -> Result<(), anyhow::Error> {
	let mut state = State::default();

	let docker = worker_lib::docker::Docker::init()?;
	let executor = worker_lib::executor::Executor::new()?;

	state.set_owned(docker);
	state.set_owned(executor);

	let deps = super::filters::Deps {
	    state: Arc::new(state),
	};
        let api = filters::runner(deps);
        let routes = api.with(warp::log("worker"));

        warp::serve(routes).run(([127, 0, 0, 1], self.port)).await;
	Ok(())
    }
}
