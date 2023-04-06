use super::task;
use crate::docker;

use common::RunContainerConfig;
use log::*;

use anyhow::anyhow;

#[async_trait::async_trait]
impl task::Task for RunContainerConfig {
    async fn run(
        self,
        context: &crate::context::Context,
        task_context: &super::TaskContext,
    ) -> Result<(), task::TaskError> {
        let mut create_params_builder = docker::CreateContainerParamsBuilder::default();
        create_params_builder
            .image(self.image)
            .name(Some(self.name))
            .mounts(self.volumes)
            .networks(self.networks)
            .ports(self.ports)
            .command(self.command)
            .restart(self.restart_policy)
            .env(self.env);

        let name = context
            .docker()
            .create_container(
                create_params_builder
                    .build()
                    .map_err(|e| anyhow!("Invalid create container params: {}", e))?,
            )
            .await?;

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

        Ok(())
    }
}
