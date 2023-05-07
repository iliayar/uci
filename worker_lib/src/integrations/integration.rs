#[async_trait::async_trait]
pub trait Integration
where
    Self: Send,
{
    async fn handle_pipeline_start(&self) -> Result<(), anyhow::Error>;
    async fn handle_pipeline_fail(&self, error: Option<String>) -> Result<(), anyhow::Error>;
    async fn handle_pipeline_done(&self) -> Result<(), anyhow::Error>;

    async fn handle_job_pending(&self, job: &str) -> Result<(), anyhow::Error>;
    async fn handle_job_progress(&self, job: &str, step: usize) -> Result<(), anyhow::Error>;
    async fn handle_job_done(&self, job: &str) -> Result<(), anyhow::Error>;
}

#[cfg(test)]
mod test {
    #[tokio::test]
    #[should_panic]
    #[allow(dead_code)]
    async fn test_dyn() {
        let integraion: Box<dyn super::Integration> = unimplemented!();
    }
}
