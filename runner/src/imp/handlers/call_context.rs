use std::{collections::HashMap, sync::Arc};

use crate::imp::{
    config,
    filters::{ContextPtr, Deps},
};

use log::*;
use tokio::sync::{mpsc, Mutex};

pub struct CallContext<PM: config::ProjectsManager> {
    pub token: Option<String>,
    pub check_permisions: bool,
    pub context: ContextPtr<PM>,
    pub ws_clients: Arc<Mutex<HashMap<String, WsClient>>>,
    pub ws: Option<WsOutput>,
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

impl<PM: config::ProjectsManager> CallContext<PM> {
    pub fn for_handler(token: Option<String>, deps: Deps<PM>) -> CallContext<PM> {
        CallContext {
            token,
            context: deps.context,
            ws_clients: deps.ws_clients,
            check_permisions: true,
            ws: None,
        }
    }

    pub async fn update_repo(&self, project_id: &str, repo_id: &str) -> Result<(), anyhow::Error> {
        self.context.update_repo(project_id, repo_id).await
    }

    pub async fn reload_config(&self) -> Result<(), anyhow::Error> {
        self.context.reload_config().await
    }

    pub async fn list_projects(&self) -> Result<Vec<config::ProjectInfo>, anyhow::Error> {
        self.context.list_projects().await
    }

    pub async fn call_trigger(
        &self,
        project_id: &str,
        trigger_id: &str,
    ) -> Result<(), anyhow::Error> {
        self.context.call_trigger(project_id, trigger_id).await
    }

    pub async fn check_permissions(
        &self,
        project_id: Option<&str>,
        action: config::ActionType,
    ) -> bool {
        if !self.check_permisions {
            return true;
        }
        if let Some(project_id) = project_id {
            match self.context.get_project_info(project_id).await {
                Ok(project_info) => project_info.check_allowed(self.token.as_ref(), action),
                Err(err) => {
                    error!(
                        "Failed to check permissions, cannot get project info: {}",
                        err
                    );
                    false
                }
            }
        } else {
            self.context
                .config()
                .await
                .check_allowed(self.token.as_ref(), action)
        }
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
