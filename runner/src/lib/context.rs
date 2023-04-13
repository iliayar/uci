use std::{borrow::BorrowMut, collections::HashMap, path::PathBuf, sync::Arc};

use super::{config, git};

use futures::Future;
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
    pub ws: Option<super::context::WsOutput>,
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
    pub async fn send<T: serde::Serialize, TR: AsRef<T>>(&self, msg: TR) {
        let content = match serde_json::to_string(msg.as_ref()) {
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

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl<PM: config::ProjectsManager> Context<PM> {
    pub async fn new(
        projects_store: config::ProjectsStore<PM>,
        worker_context: Option<worker_lib::context::Context>,
        config_path: PathBuf,
        env: String,
    ) -> Result<Context<PM>, ContextError> {
        let config = load_config_impl(config_path.clone(), env.clone()).await?;
        Ok(Context {
            config: Mutex::new(Arc::new(config)),
            ws_clients: Mutex::new(HashMap::default()),
            ws: None,
            worker_context,
            config_path,
            env,
            projects_store,
        })
    }

    pub async fn init(&self) -> Result<(), anyhow::Error> {
        self.init_projects().await?;
        Ok(())
    }

    pub async fn config(&self) -> Arc<config::ServiceConfig> {
        return self.config.lock().await.clone();
    }

    pub async fn reload_config(&self) -> Result<(), ContextError> {
        *self.config.lock().await =
            Arc::new(load_config_impl(self.config_path.clone(), self.env.clone()).await?);
        Ok(())
    }

    pub async fn init_projects(&self) -> Result<(), ContextError> {
        let mut state = config::State::default();
        let config = self.config.lock().await.clone();
        state.set(config.as_ref());
        state.set_named("env", &self.env);
        self.projects_store.init(&state).await?;
        Ok(())
    }

    pub async fn reload_project(&self, project_id: &str) -> Result<(), ContextError> {
        let mut state = config::State::default();
        let config = self.config.lock().await.clone();
        state.set(config.as_ref());
        state.set_named("env", &self.env);
        self.projects_store
            .reload_project(&state, project_id)
            .await?;
        Ok(())
    }

    pub async fn list_projects(&self) -> Result<Vec<config::ProjectInfo>, ContextError> {
        let mut state = config::State::default();
        let config = self.config.lock().await.clone();
        state.set(config.as_ref());
        state.set_named("env", &self.env);
        Ok(self.projects_store.list_projects(&state).await?)
    }

    pub async fn get_project_info(
        &self,
        project_id: &str,
    ) -> Result<config::ProjectInfo, ContextError> {
        let mut state = config::State::default();
        let config = self.config.lock().await.clone();
        state.set(config.as_ref());
        state.set_named("env", &self.env);
        Ok(self
            .projects_store
            .get_project_info(&state, project_id)
            .await?)
    }

    pub async fn init_ws(&mut self) -> String {
        let client_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::unbounded_channel();
        let client = WsClient { rx };
        self.ws_clients
            .lock()
            .await
            .insert(client_id.clone(), client);
        debug!("New ws client registerd: {}", client_id);
        let ws = WsOutput { client_id, tx };
        let client_id = ws.client_id.clone();
        self.ws = Some(ws);
        client_id
    }

    pub async fn finish_ws(&mut self) {
        if let Some(ws) = self.ws.take() {
            self.ws_clients.lock().await.remove(&ws.client_id);
        }
    }
}

async fn load_config_impl(
    config_path: PathBuf,
    env: String,
) -> Result<config::ServiceConfig, ContextError> {
    let mut context = config::State::default();
    context.set_named("service_config", &config_path);
    let config = config::ServiceConfig::load(&context).await?;

    info!("Loaded config: {:#?}", config);
    Ok(config)
}
