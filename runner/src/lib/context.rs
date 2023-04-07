use std::{borrow::BorrowMut, collections::HashMap, path::PathBuf, sync::Arc};

use super::{config, git};

use log::*;
use thiserror::Error;
use tokio::sync::Mutex;

pub struct Context {
    config_path: PathBuf,
    config: Mutex<Arc<config::Config>>,
    env: String,
}

#[derive(Debug, Error)]
pub enum ContextError {
    #[error("Failed to load config: {0}")]
    ConfigLoadError(#[from] config::LoadConfigError),

    #[error("Failed to execute config: {0}")]
    ConfigExecutionError(#[from] config::ExecutionError),

    #[error("Git error: {0}")]
    GitError(#[from] git::GitError),
}

impl Context {
    pub async fn new(config_path: PathBuf, env: String) -> Result<Context, ContextError> {
        let config = load_config_impl(config_path.clone(), &env).await?;
        let context = Context {
            config: Mutex::new(Arc::new(config)),
            config_path,
            env,
        };

        Ok(context)
    }

    pub async fn config(&self) -> Arc<config::Config> {
        return self.config.lock().await.clone();
    }

    pub async fn reload_config(&self) -> Result<(), ContextError> {
        let config = load_config_impl(self.config_path.clone(), &self.env).await?;
        info!("Config reloaded {:#?}", config);

        *self.config.lock().await = Arc::new(config);

        Ok(())
    }
}

async fn load_config_impl(config_path: PathBuf, env: &str) -> Result<config::Config, ContextError> {
    let config = config::Config::load(config_path.clone(), env).await?;
    info!("Loaded config: {:#?}", config);

    config.clone_missing_repos().await?;

    Ok(config)
}
