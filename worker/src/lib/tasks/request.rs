use common::RequestConfig;

use anyhow::anyhow;
use log::*;

use super::error::TaskError;

pub async fn run_request(config: &RequestConfig) -> Result<(), TaskError> {
    let mut client = match &config.method {
        common::RequestMethod::Post => reqwest::Client::new().post(&config.url),
        common::RequestMethod::Get => reqwest::Client::new().post(&config.url),
    };

    if let Some(body) = &config.body {
        client = client.body(body.clone());
    }

    let response = client.send().await?;

    info!("Response: {:?}", response);

    response.error_for_status()?;

    Ok(())
}
