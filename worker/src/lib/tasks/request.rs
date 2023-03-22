use common::RequestConfig;

use log::*;
use anyhow::anyhow;

use super::error::TaskError;

pub async fn run_request(config: &RequestConfig) -> Result<(), TaskError> {
    let mut client = match &config.method {
	Post => reqwest::Client::new().post(&config.url),
	Get => reqwest::Client::new().post(&config.url),
    };

    if let Some(body) = &config.body {
	client = client.body(body.clone());
    }

    let response = client.send().await?;

    info!("Response: {:?}", response);

    response.error_for_status()?;

    Ok(())
}
