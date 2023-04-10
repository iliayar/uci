use std::{borrow::BorrowMut, collections::HashMap, path::PathBuf, sync::Arc};

use super::{config, git};

use log::*;
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

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
        let config = config::Config::preload(config_path.clone(), env.clone()).await?;
        config.clone_missing_repos().await?;
        let config = config.load().await?;
        info!("Loaded config: {:#?}", config);

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

    pub async fn preload_config(&self) -> Result<super::config::ConfigPreload, ContextError> {
        Ok(config::Config::preload(self.config_path.clone(), self.env.clone()).await?)
    }

    pub async fn load_config(
        &self,
        config: config::ConfigPreload,
    ) -> Result<(), ContextError> {
        config.clone_missing_repos().await?;

        let config = config.load().await?;
        info!("Loaded config: {:#?}", config);

        *self.config.lock().await = Arc::new(config);

        Ok(())
    }
}

pub struct ExecutionContext {
    check_permisions: bool,
    token: Option<String>,
    worker_context: Option<worker_lib::context::Context>,
    config: Arc<super::config::Config>,
}

impl ExecutionContext {
    pub fn check_allowed_global(&self, action: super::config::ActionType) -> bool {
        self.config.service_config.check_allowed::<_, &str>(
            self.token.as_ref().clone(),
            None,
            action,
        )
    }

    pub fn check_allowed<S: AsRef<str>>(
        &self,
        project: S,
        action: super::config::ActionType,
    ) -> bool {
        self.config.service_config.check_allowed::<_, &str>(
            self.token.as_ref().clone(),
            Some(project.as_ref()),
            action,
        )
    }
}
