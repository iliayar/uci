use reqwest::Url;

use futures_util::{stream::SplitStream, FutureExt, StreamExt};
use serde::Deserialize;

use log::*;
use termion::{clear, color, style};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::execute::ExecuteError;

pub struct WsClient {
    rx: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
}

impl WsClient {
    pub async fn receive<T: for<'a> Deserialize<'a>>(&mut self) -> Option<T> {
        let message = self.rx.next().await;
        debug!("Receive message: {:?}", message);
        self.receive_impl(message).await
    }

    pub async fn try_receive<T: for<'a> Deserialize<'a>>(&mut self) -> Option<T> {
        if let Some(message) = self.rx.next().now_or_never() {
            self.receive_impl(message).await
        } else {
            None
        }
    }

    async fn receive_impl<T: for<'a> Deserialize<'a>>(
        &mut self,
        message: Option<
            Result<tokio_tungstenite::tungstenite::Message, tokio_tungstenite::tungstenite::Error>,
        >,
    ) -> Option<T> {
        let message = match message? {
            Ok(message) => message,
            Err(err) => {
                warn!("WebSocket error: {}", err);
                return None;
            }
        };

        debug!("Matching message type: {:?}", message);
        let message = match message {
            tokio_tungstenite::tungstenite::Message::Text(message) => message,
            tokio_tungstenite::tungstenite::Message::Close(_) => {
                return None;
            }
            _ => {
                warn!("Unknown WebSocket message type: {}", message);
                return None;
            }
        };

        match serde_json::from_str(&message) {
            Ok(value) => Some(value),
            Err(err) => {
                error!("Failed to decode WebSocket message: {}", err);
                None
            }
        }
    }
}

pub async fn ws(
    config: &super::config::Config,
    client_id: String,
) -> Result<WsClient, ExecuteError> {
    debug!("Connecting with client id {}", client_id);
    let runner_url = config
        .ws_runner_url
        .as_ref()
        .expect("ws_runner_url is not set");
    let url = Url::parse(&format!("{}/ws/{}", runner_url, client_id)).unwrap();

    let (ws_stream, _) = tokio_tungstenite::connect_async(url)
        .await
        .map_err(|err| ExecuteError::Fatal(format!("Failed to connect to socket: {}", err)))?;

    let (_, read) = ws_stream.split();

    if let Err(err) = ctrlc::set_handler(move || {
        println!(
            "{}{}Stop watching run{}",
            clear::CurrentLine,
            color::Fg(color::Yellow),
            style::Reset
        );
        println!("Run id: {}{}{}", style::Bold, client_id, style::Reset);
        // TODO: Print command to continue watch run
        std::process::exit(0);
    }) {
        error!("Failed to set Ctrl-C handler: {}", err);
    }

    debug!("WS Connected");

    Ok(WsClient { rx: read })
}
