use std::{collections::HashMap, sync::Arc};

use crate::imp::{
    config,
    filters::{ContextPtr, Deps},
};

use log::*;
use tokio::sync::{mpsc, Mutex};

pub type Runs = Arc<Mutex<HashMap<String, Arc<RunContext>>>>;
pub type WsClientReciever = mpsc::UnboundedReceiver<Result<warp::ws::Message, warp::Error>>;

type WsSender = mpsc::UnboundedSender<Result<warp::ws::Message, warp::Error>>;

const ENABLE_BUFFERING: bool = true;

pub struct CallContext<PM: config::ProjectsManager> {
    pub token: Option<String>,
    pub check_permisions: bool,
    pub context: ContextPtr<PM>,
    pub runs: Arc<Mutex<HashMap<String, Arc<RunContext>>>>,
    pub run_context: Option<Arc<RunContext>>,
}

pub struct RunContext {
    pub id: String,
    pub txs: Mutex<Vec<WsSender>>,
    pub buffer: Mutex<Vec<String>>,
    pub enable_buffering: bool,
}

impl RunContext {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            txs: Mutex::new(Vec::new()),
            buffer: Mutex::new(Vec::new()),
            enable_buffering: false,
        }
    }

    pub fn new_buffered() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            txs: Mutex::new(Vec::new()),
            buffer: Mutex::new(Vec::new()),
            enable_buffering: ENABLE_BUFFERING,
        }
    }
}

impl RunContext {
    pub async fn send<T: serde::Serialize, TR: AsRef<T>>(&self, msg: TR) {
        let content = match serde_json::to_string(msg.as_ref()) {
            Err(err) => {
                error!("Failed to encode msg for ws: {}", err);
                return;
            }
            Ok(content) => content,
        };

        let mut txs = self.txs.lock().await;
        if txs.is_empty() {
            debug!("No ws clients to send message to");
            if self.enable_buffering {
                debug!("Buffering message {}", content);
                self.buffer.lock().await.push(content);
            }
            return;
        }

        let mut need_update = false;
        for tx in txs.iter() {
            // FIXME: Actually it may be true event if client close
            // connection. Maybe do it depending on errors?
            //
            // See handlers::ws::ws_client_connection
            if tx.is_closed() {
                need_update = true;
                continue;
            }

            debug!("Sending ws message: {}", content);
            if let Err(err) = tx.send(Ok(warp::ws::Message::text(content.clone()))) {
                error!("Failed to send ws message {}", err);
                need_update = true;
            }
        }

        if need_update {
            let old_count = txs.len();
            let mut old_txs = Vec::new();
            std::mem::swap(&mut old_txs, txs.as_mut());
            *txs = old_txs.into_iter().filter(|tx| !tx.is_closed()).collect();
            let new_count = txs.len();

            debug!(
                "Ws clients number changed for run {}: {} -> {}",
                self.id, old_count, new_count
            );
        }
    }

    async fn make_client_receiver(&self) -> WsClientReciever {
        let (tx, rx) = mpsc::unbounded_channel();

	// NOTE: Avoiding dead lock in send_buffered
        let (old_count, new_count) = {
            let mut txs = self.txs.lock().await;
            let old_count = txs.len();
            txs.push(tx);
            let new_count = txs.len();
	    (old_count, new_count)
        };

        debug!(
            "Ws clients number changed for run {}: {} -> {}",
            self.id, old_count, new_count
        );
        self.send_buffered().await;
        rx
    }

    async fn send_buffered(&self) {
        // NOTE: Intentionally send buffered messages only to first client
        if !self.enable_buffering || self.buffer.lock().await.is_empty() {
            return;
        }

        let tx = {
            let txs = self.txs.lock().await;
            assert!(!txs.is_empty());
            txs[0].clone()
        };

        let mut buffer = Vec::new();
        std::mem::swap(self.buffer.lock().await.as_mut(), &mut buffer);
        for msg in buffer {
            debug!("Sending buffered ws message: {}", msg);
            if let Err(err) = tx.send(Ok(warp::ws::Message::text(msg))) {
                error!("Failed to send buffered ws message {}", err);
            }
        }
    }
}

impl<PM: config::ProjectsManager> CallContext<PM> {
    pub fn for_handler(token: Option<String>, deps: Deps<PM>) -> CallContext<PM> {
        CallContext {
            token,
            context: deps.context,
            runs: deps.runs,
            check_permisions: true,
            run_context: None,
        }
    }

    pub async fn update_repo(&self, project_id: &str, repo_id: &str) -> Result<(), anyhow::Error> {
        let mut state = config::State::default();
        if let Some(run_context) = self.run_context.as_ref() {
            state.set(run_context.as_ref());
        }
        self.context.update_repo(&state, project_id, repo_id).await
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
        let mut state = config::State::default();
        if let Some(run_context) = self.run_context.as_ref() {
            state.set(run_context.as_ref());
        }
        self.context.call_trigger(&state, project_id, trigger_id).await
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

    pub async fn init_run(&mut self) -> String {
        self.init_run_impl(RunContext::new()).await
    }

    pub async fn init_run_buffered(&mut self) -> String {
        self.init_run_impl(RunContext::new_buffered()).await
    }

    async fn init_run_impl(&mut self, run_context: RunContext) -> String {
        let run_context = Arc::new(run_context);
        self.runs
            .lock()
            .await
            .insert(run_context.id.clone(), run_context.clone());
        self.run_context = Some(run_context.clone());
        debug!("New ws run registerd: {}", run_context.id);
        run_context.id.clone()
    }

    pub async fn make_out_channel(&self, run_id: String) -> Option<WsClientReciever> {
        if let Some(run_context) = self.runs.lock().await.get(&run_id) {
            Some(run_context.make_client_receiver().await)
        } else {
            error!("Trying get not existing run {}", run_id);
            None
        }
    }

    pub async fn finish_run(&mut self) {
        if let Some(run_context) = self.run_context.take() {
            self.runs.lock().await.remove(&run_context.id);
        }
    }
}
