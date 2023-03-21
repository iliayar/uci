use std::{path::PathBuf, pin::Pin, sync::Arc};

use thiserror::Error;
use warp::{Filter, Future};

use log::*;

use clap::Parser;

use crate::lib::{git, utils::expand_home};

use super::{
    config::{Config, ConfigError},
    filters,
    git::GitError, context::{Context, ContextError},
};

#[derive(Parser, Debug)]
#[command(about)]
struct Args {
    /// Path to directory with configs
    #[arg(short, long)]
    config: PathBuf,

    /// TCP port to run on
    #[arg(short, long, default_value_t = 3002)]
    port: u16,
}

pub struct App {
    context: Context,
    port: u16,
}

#[derive(Error, Debug)]
pub enum RunnerError {
    #[error("Failed to load config: {0}")]
    ConfigLoadError(#[from] ConfigError),

    #[error("Failed to create context: {0}")]
    ContextError(#[from] ContextError),
}

impl App {
    pub async fn init() -> Result<App, RunnerError> {
        pretty_env_logger::init();

	let args = Args::parse();

        let app = App {
            context: Context::new(args.config).await?,
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

        let api = filters::runner(self.context);
        let routes = api.with(warp::log("runner"));
        warp::serve(routes).run(([127, 0, 0, 1], self.port)).await;
    }

    async fn clone_missing_repos(&self) -> Result<(), RunnerError> {
	Ok(self.context.clone_missing_repos().await?)
    }
}
