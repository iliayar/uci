#[async_trait::async_trait]
impl super::Task for common::ParallelConfig {
    async fn run(
        self,
        context: &crate::context::Context,
        task_context: &super::TaskContext,
    ) -> Result<(), super::TaskError> {
        let mut tasks = Vec::new();
        for step in self.steps.into_iter() {
            tasks.push(step.run(context, task_context));
        }
        futures::future::try_join_all(tasks).await?;
        Ok(())
    }
}
