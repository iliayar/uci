use std::{path::PathBuf, pin::Pin};

use thiserror::Error;
use warp::{Filter, Future};

use log::*;

use crate::lib::{git, utils::expand_home};

use super::{
    config::{Config, ConfigError},
    filters,
    git::GitError, context::{Context, ContextError},
};

pub struct App {
    context: Context,
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

	let config = Config::load("../tests/static/config".into()).await?;
        info!("Loaded config: {:?}", config);

        let app = App {
            context: Context::new(config),
        };

        Ok(app)
    }

    pub async fn run(self) {
        let api = filters::runner();
        let routes = api.with(warp::log("runner"));

        match self.clone_missing_repos().await {
            Ok(_) => {}
            Err(err) => {
                error!("Failed to clone missing repos, exiting: {}", err);
                return;
            }
        }

        warp::serve(routes).run(([127, 0, 0, 1], 3002)).await;
    }

    async fn clone_missing_repos(&self) -> Result<(), RunnerError> {
	Ok(self.context.clone_missing_repos().await?)
    }
}
