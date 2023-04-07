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
    port: u16,
    config: PathBuf,
    worker: bool,
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

        Ok(App {
            port: args.port,
            config: args.config,
            worker: args.worker,
        })
    }

    pub async fn run(self) {
        match self.run_impl().await {
            Ok(_) => {}
            Err(err) => {
                error!("Failed to run app, exiting: {}", err);
            }
        }
    }

    async fn run_impl(self) -> Result<(), RunnerError> {
        let worker_context = if self.worker {
            let docker = worker_lib::docker::Docker::init()?;
            Some(worker_lib::context::Context::new(docker))
        } else {
            None
        };

        let context = context::Context::new(self.config).await?;

        context
            .config()
            .await
            .autostart(worker_context.clone())
            .await
            .map_err(|err| Into::<context::ContextError>::into(err))?;

        let api = filters::runner(context, worker_context);
        let routes = api.with(warp::log("runner"));
        warp::serve(routes).run(([127, 0, 0, 1], self.port)).await;
        Ok(())
    }
}
