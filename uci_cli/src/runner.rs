use reqwest::{header, Url};

use futures_util::StreamExt;
use serde::Deserialize;

use log::*;

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

pub fn get<S: AsRef<str>>(
    config: &super::config::Config,
    path: S,
) -> Result<reqwest::RequestBuilder, anyhow::Error> {
    let runner_url = config.runner_url.as_ref().expect("runner_url is not set");
    Ok(call_runner(config)?.get(format!("{}{}", runner_url, path.as_ref())))
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
                let error_response: common::runner::ErrorResponse =
                    response.json().await.map_err(Into::<anyhow::Error>::into)?;
                Err(super::execute::ExecuteError::Fatal(error_response.message))
            }
        }
        Err(err) => Err(Into::<anyhow::Error>::into(err).into()),
    }
}

pub async fn ws<T: for<'a> Deserialize<'a>>(
    config: &super::config::Config,
    client_id: String,
) -> impl StreamExt<Item = T> {
    let runner_url = config
        .ws_runner_url
        .as_ref()
        .expect("runner_url is not set");
    let url = Url::parse(&format!("{}/ws/{}", runner_url, client_id)).unwrap();

    let (ws_stream, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("Failed to connect");

    let (_, read) = ws_stream.split();

    read.filter_map(|msg| async move {
        match msg {
            Ok(msg) => match msg {
                tokio_tungstenite::tungstenite::Message::Text(content) => {
                    match serde_json::from_str(&content) {
                        Err(err) => {
                            error!("Failed to decode WebSocket message: {}", err);
                            None
                        }
                        Ok(value) => Some(value),
                    }
                }
                _ => None,
            },
            Err(err) => {
                warn!("WebSocket error: {}", err);
                None
            }
        }
    })
}
