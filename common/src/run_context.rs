use tokio::sync::{mpsc, Mutex};

const ENABLE_BUFFERING: bool = true;

use log::*;

pub struct WsMessage {
    message: String,
}

impl WsMessage {
    pub fn text(message: String) -> WsMessage {
        WsMessage { message }
    }
}

impl ToString for WsMessage {
    fn to_string(&self) -> String {
        self.message.clone()
    }
}

pub type WsClientReciever = mpsc::UnboundedReceiver<Result<WsMessage, anyhow::Error>>;
pub type WsSender = mpsc::UnboundedSender<Result<WsMessage, anyhow::Error>>;

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

    pub fn empty() -> Self {
        Self {
            id: "(none)".to_string(),
            txs: Mutex::new(Vec::new()),
            buffer: Mutex::new(Vec::new()),
            enable_buffering: false,
        }
    }
}

impl Default for RunContext {
    fn default() -> Self {
        Self::new()
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
            if let Err(err) = tx.send(Ok(WsMessage::text(content.clone()))) {
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

    pub async fn has_clients(&self) -> bool {
	!self.txs.lock().await.is_empty()
    }

    pub async fn make_client_receiver(&self) -> WsClientReciever {
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
            if let Err(err) = tx.send(Ok(WsMessage::text(msg))) {
                error!("Failed to send buffered ws message {}", err);
            }
        }
    }
}
