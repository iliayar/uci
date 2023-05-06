use reqwest::{header, Url};

use futures_util::{stream::SplitStream, FutureExt, StreamExt};
use serde::{Deserialize, Serialize};

use log::*;
use termion::{clear, color, style};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::execute::ExecuteError;

fn call_runner(config: &super::config::Config) -> Result<reqwest::Client, anyhow::Error> {
    let mut headers = header::HeaderMap::new();

    if let Some(token) = config.token.as_ref() {
        let mut auth_value = header::HeaderValue::from_str(&format!("Api-Key {}", token))?;
        auth_value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth_value);
    }

    Ok(reqwest::Client::builder()
        .default_headers(headers)
        .build()?)
}

pub fn post<S: AsRef<str>>(
    config: &super::config::Config,
    path: S,
) -> Result<reqwest::RequestBuilder, anyhow::Error> {
    let runner_url = config.runner_url.as_ref().expect("runner_url is not set");
    Ok(call_runner(config)?.post(format!("{}{}", runner_url, path.as_ref())))
}

pub fn post_body<S: AsRef<str>, T: Serialize>(
    config: &super::config::Config,
    path: S,
    body: &T,
) -> Result<reqwest::RequestBuilder, anyhow::Error> {
    let runner_url = config.runner_url.as_ref().expect("runner_url is not set");
    Ok(call_runner(config)?
        .post(format!("{}{}", runner_url, path.as_ref()))
        .json(body))
}

pub fn get<S: AsRef<str>>(
    config: &super::config::Config,
    path: S,
) -> Result<reqwest::RequestBuilder, anyhow::Error> {
    let runner_url = config.runner_url.as_ref().expect("runner_url is not set");
    Ok(call_runner(config)?.get(format!("{}{}", runner_url, path.as_ref())))
}

pub fn get_query<S: AsRef<str>, T: serde::Serialize>(
    config: &super::config::Config,
    path: S,
    query: &T,
) -> Result<reqwest::RequestBuilder, anyhow::Error> {
    let runner_url = config.runner_url.as_ref().expect("runner_url is not set");
    Ok(call_runner(config)?
        .get(format!("{}{}", runner_url, path.as_ref()))
        .query(query))
}

pub fn get_query_body<S: AsRef<str>, T: serde::Serialize, E: serde::Serialize>(
    config: &super::config::Config,
    path: S,
    query: &T,
    body: &E,
) -> Result<reqwest::RequestBuilder, anyhow::Error> {
    let runner_url = config.runner_url.as_ref().expect("runner_url is not set");
    Ok(call_runner(config)?
        .get(format!("{}{}", runner_url, path.as_ref()))
        .query(query)
        .json(body))
}

pub async fn json<T: for<'a> Deserialize<'a>>(
    response: reqwest::Result<reqwest::Response>,
) -> Result<T, super::execute::ExecuteError> {
    match response {
        Ok(response) => {
            info!("Get reponse with status {:?}", response.status());
            if response.status().is_success() {
                Ok(response.json().await.map_err(Into::<anyhow::Error>::into)?)
            } else {
                let status = response.status();
                let text = response.text().await.map_err(Into::<anyhow::Error>::into)?;
                match serde_json::from_str::<common::runner::ErrorResponse>(&text) {
                    Ok(error_response) => {
                        Err(super::execute::ExecuteError::Fatal(error_response.message))
                    }
                    Err(err) => Err(super::execute::ExecuteError::Fatal(format!(
                        "Failed to parse response as json ({}). Got {}: {}",
                        err, status, text
                    ))),
                }
            }
        }
        Err(err) => Err(Into::<anyhow::Error>::into(err).into()),
    }
}

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

pub mod api {
    use crate::{config::Config, execute::ExecuteError};

    pub async fn list_services(
        config: &Config,
        project_id: String,
    ) -> Result<common::runner::ServicesListResponse, ExecuteError> {
        let query = common::runner::ListServicesQuery { project_id };
        let response = crate::runner::get_query(config, "/projects/services/list", &query)?
            .send()
            .await;
        crate::runner::json(response).await
    }

    pub async fn list_runs(
        config: &Config,
        project_id: Option<String>,
        pipeline_id: Option<String>,
    ) -> Result<common::runner::ListRunsResponse, ExecuteError> {
        let query = common::runner::ListRunsRequestQuery {
            project_id,
            pipeline_id,
        };
        let response = crate::runner::get_query(config, "/runs/list", &query)?
            .send()
            .await;
        crate::runner::json(response).await
    }

    pub async fn actions_list(
        config: &Config,
        project_id: String,
    ) -> Result<common::runner::ActionsListResponse, ExecuteError> {
        let query = common::runner::ListActionsQuery { project_id };
        let response = crate::runner::get_query(config, "/projects/actions/list", &query)?
            .send()
            .await;
        crate::runner::json(response).await
    }

    pub async fn projects_list(
        config: &Config,
    ) -> Result<common::runner::ProjectsListResponse, ExecuteError> {
        let response = crate::runner::get(config, "/projects/list")?.send().await;
        crate::runner::json(response).await
    }

    pub async fn repos_list(
        config: &Config,
        project_id: String,
    ) -> Result<common::runner::ReposListResponse, ExecuteError> {
        let query = common::runner::ListReposQuery { project_id };
        let response = crate::runner::get_query(config, "/projects/repos/list", &query)?
            .send()
            .await;
        crate::runner::json(response).await
    }

    pub async fn upload(
        config: &Config,
	data: Vec<u8>,
    ) -> Result<common::runner::UploadResponse, ExecuteError> {
	let file_part = reqwest::multipart::Part::bytes(data);
	let form = reqwest::multipart::Form::new().part("file", file_part);
        let response = crate::runner::post(config, "/upload")?.multipart(form).send().await;
        crate::runner::json(response).await
    }
}
