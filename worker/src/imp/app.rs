use thiserror::Error;
use warp::Filter;

use super::filters;
use worker_lib::context::Context;
use worker_lib::docker;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(about)]
struct Args {
    /// TCP port to run on
    #[arg(short, long, default_value_t = 3001)]
    port: u16,
}

pub struct App {
    context: Context,
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
        let context = Context::new(docker);
        let app = App {
            context,
            port: args.port,
        };

        pretty_env_logger::init();

        Ok(app)
    }

    pub async fn run(self) {
        let api = filters::runner(self.context);
        let routes = api.with(warp::log("worker"));

        warp::serve(routes).run(([127, 0, 0, 1], self.port)).await;
    }
}
