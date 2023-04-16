use crate::docker::{self, Docker};

use anyhow::anyhow;
use common::state::State;

#[async_trait::async_trait]
impl super::Task for common::StopContainerConfig {
    async fn run(self, state: &State) -> Result<(), anyhow::Error> {
        let docker: &Docker = state.get()?;
        let mut stop_params_builder = docker::StopContainerParamsBuilder::default();
        stop_params_builder.name(self.name);

        docker
            .stop_container(
                stop_params_builder
                    .build()
                    .map_err(|e| anyhow!("Invalid stop container params: {}", e))?,
            )
            .await?;

        Ok(())
    }
}
