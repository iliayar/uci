use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct GitLabIntegration {
    token: String,
}

impl GitLabIntegration {}

#[async_trait::async_trait]
impl super::integration::Integration for GitLabIntegration {
    async fn handle_pipeline_start(&self) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn handle_pipeline_fail(&self, error: Option<String>) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn handle_pipeline_done(&self) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn handle_job_pending(&self, job: &str) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn handle_job_progress(&self, job: &str, step: usize) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn handle_job_done(&self, job: &str) -> Result<(), anyhow::Error> {
        Ok(())
    }
}
