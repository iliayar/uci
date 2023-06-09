use common::{state::State, RequestConfig};

use log::*;

use super::task;

#[async_trait::async_trait]
impl task::Task for RequestConfig {
    async fn run(self, state: &State) -> Result<(), anyhow::Error> {
        let mut client = match &self.method {
            common::RequestMethod::Post => reqwest::Client::new().post(&self.url),
            common::RequestMethod::Get => reqwest::Client::new().post(&self.url),
        };

        if let Some(body) = &self.body {
            client = client.body(body.clone());
        }

        let response = client.send().await?;

        info!("Response: {:?}", response);

        response.error_for_status()?;

        Ok(())
    }
}
