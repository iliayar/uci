use std::{path::PathBuf, pin::Pin, sync::Arc};

use thiserror::Error;
use warp::{Filter, Future};

use log::*;

use clap::Parser;

use super::{config, context, filters, git};

#[derive(Parser, Debug)]
#[command(about)]
struct Args {
    /// Path to directory with configs
    #[arg(short, long)]
    config: PathBuf,

    /// TCP port to run on
    #[arg(short, long, default_value_t = 3002)]
    port: u16,

    /// Do not use external worker, run pipelines in the same process
    #[arg(long, default_value_t = false)]
    worker: bool,
}

pub struct App {
    context: context::Context,
    worker_context: Option<worker_lib::context::Context>,
    port: u16,
}

#[derive(Error, Debug)]
pub enum RunnerError {
    #[error("Failed to load config: {0}")]
    ConfigLoadError(#[from] config::LoadConfigError),

    #[error("Failed to create context: {0}")]
    ContextError(#[from] context::ContextError),

    #[error("Failed to init docker: {0}")]
    DockerError(#[from] worker_lib::docker::DockerError),
}

impl App {
    pub async fn init() -> Result<App, RunnerError> {
        pretty_env_logger::init();

        let args = Args::parse();

        let worker_context = if args.worker {
            let docker = worker_lib::docker::Docker::init()?;
            Some(worker_lib::context::Context::new(docker))
        } else {
            None
        };

        let app = App {
            context: context::Context::new(args.config).await?,
            worker_context,
            port: args.port,
        };

        Ok(app)
    }

    pub async fn run(self) {
        match self.clone_missing_repos().await {
            Ok(_) => {}
            Err(err) => {
                error!("Failed to clone missing repos, exiting: {}", err);
                return;
            }
        }

        let api = filters::runner(self.context, self.worker_context);
        let routes = api.with(warp::log("runner"));
        warp::serve(routes).run(([127, 0, 0, 1], self.port)).await;
    }

    async fn clone_missing_repos(&self) -> Result<(), RunnerError> {
        Ok(self.context.clone_missing_repos().await?)
    }
}
