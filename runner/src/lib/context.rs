use std::{borrow::BorrowMut, collections::HashMap, path::PathBuf, sync::Arc};

use super::{config, git};

use log::*;
use thiserror::Error;
use tokio::sync::{mpsc, Mutex};

pub struct Context<PM: config::ProjectsManager> {
    pub config_path: PathBuf,
    pub env: String,
    config: Mutex<Arc<config::ServiceConfig>>,
    pub projects_store: config::ProjectsStore<PM>,
    pub ws_clients: Mutex<HashMap<String, WsClient>>,
    pub worker_context: Option<worker_lib::context::Context>,
}

pub struct WsClient {
    pub rx: mpsc::UnboundedReceiver<Result<warp::ws::Message, warp::Error>>,
}

#[derive(Clone)]
pub struct WsOutput {
    pub client_id: String,
    pub tx: mpsc::UnboundedSender<Result<warp::ws::Message, warp::Error>>,
}

impl WsOutput {
    pub async fn send<T: serde::Serialize>(&self, msg: T) {
        let content = match serde_json::to_string(&msg) {
            Err(err) => {
                error!("Failed to encode msg for ws: {}", err);
                return;
            }
            Ok(content) => content,
        };
        if let Err(err) = self.tx.send(Ok(warp::ws::Message::text(content))) {
            error!("Failed to send ws message {}", err);
        }
    }
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

impl<PM: config::ProjectsManager> Context<PM> {
    pub async fn new(
        projects_store: config::ProjectsStore<PM>,
        worker_context: Option<worker_lib::context::Context>,
        config_path: PathBuf,
        env: String,
    ) -> Result<Context<PM>, ContextError> {
        let config = load_config_impl(config_path.clone(), env.clone()).await?;
        let context = Context {
            config: Mutex::new(Arc::new(config)),
            ws_clients: Mutex::new(HashMap::default()),
            worker_context,
            config_path,
            env,
            projects_store,
        };

        Ok(context)
    }

    pub async fn config(&self) -> Arc<config::ServiceConfig> {
        return self.config.lock().await.clone();
    }

    pub async fn reload_config(&self) -> Result<(), ContextError> {
        *self.config.lock().await =
            Arc::new(load_config_impl(self.config_path.clone(), self.env.clone()).await?);
        Ok(())
    }
}

async fn load_config_impl(
    config_path: PathBuf,
    env: String,
) -> Result<config::ServiceConfig, ContextError> {
    let mut context = config::LoadContext::default();
    context.set_named("service_config", &config_path);
    let config = config::ServiceConfig::load(&context).await?;

    info!("Loaded config: {:#?}", config);
    Ok(config)
}
