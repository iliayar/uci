use crate::docker;

use anyhow::anyhow;

#[async_trait::async_trait]
impl super::Task for common::StopContainerConfig {
    async fn run(
        self,
        context: &crate::context::Context,
        task_context: &super::TaskContext,
    ) -> Result<(), super::TaskError> {
        let mut stop_params_builder = docker::StopContainerParamsBuilder::default();
        stop_params_builder.name(self.name);

        context
            .docker()
            .stop_container(
                stop_params_builder
                    .build()
                    .map_err(|e| anyhow!("Invalid stop container params: {}", e))?,
            )
            .await?;

        Ok(())
    }
}
