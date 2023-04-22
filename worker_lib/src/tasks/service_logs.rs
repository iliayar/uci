use crate::docker::{self, Docker};

use anyhow::anyhow;
use common::state::State;

#[async_trait::async_trait]
impl super::Task for common::ServiceLogsConfig {
    async fn run(self, state: &State) -> Result<(), anyhow::Error> {
        let docker: &Docker = state.get()?;
        let mut params = docker::LogsParamsBuilder::default();
        params
            .container(self.container)
            .follow(self.follow)
            .tail(self.tail);

        docker
            .logs(
                state,
                params
                    .build()
                    .map_err(|e| anyhow!("Invalid stop container params: {}", e))?,
            )
            .await?;

        Ok(())
    }
}
