use common::state::State;

#[async_trait::async_trait]
impl super::Task for common::ParallelConfig {
    async fn run(self, state: &State) -> Result<(), anyhow::Error> {
        let mut tasks = Vec::new();
        for step in self.steps.into_iter() {
            tasks.push(step.run(state));
        }
        futures::future::try_join_all(tasks).await?;
        Ok(())
    }
}
