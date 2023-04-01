#[async_trait::async_trait]
impl super::Task for common::DeployConfig {
    async fn run(self, context: &crate::context::Context) -> Result<(), super::TaskError> {
        self.build_config.run(context).await?;
        self.stop_config.run(context).await?;
        self.run_config.run(context).await?;

        Ok(())
    }
}
