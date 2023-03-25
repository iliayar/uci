use super::task;
use crate::lib::docker;

use common::RunContainerConfig;
use log::*;

use anyhow::anyhow;

#[async_trait::async_trait]
impl task::Task for RunContainerConfig {
    async fn run(self, context: &crate::lib::context::Context) -> Result<(), task::TaskError> {
        let mut create_params_builder = docker::CreateContainerParamsBuilder::default();
        create_params_builder
            .image(self.image)
            .name(Some(self.name));

        let name = context
            .docker()
            .create_container(
                create_params_builder
                    .build()
                    .map_err(|e| anyhow!("Invalid create container params: {}", e))?,
            )
            .await?;
        info!("Created container '{}'", name);

        let mut start_params_builder = docker::StartContainerParamsBuilder::default();
        start_params_builder.name(name.clone());

        context
            .docker()
            .start_container(
                start_params_builder
                    .build()
                    .map_err(|e| anyhow!("Invalid start container params: {}", e))?,
            )
            .await?;
        info!("Container started '{}'", name);

        Ok(())
    }
}
