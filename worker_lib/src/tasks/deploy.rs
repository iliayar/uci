#[async_trait::async_trait]
impl super::Task for common::DeployConfig {
    async fn run(
        self,
        context: &crate::context::Context,
        task_context: &super::TaskContext,
    ) -> Result<(), super::TaskError> {
        self.build_config.run(context, task_context).await?;
        self.stop_config.run(context, task_context).await?;
        self.run_config.run(context, task_context).await?;

        Ok(())
    }
}
