use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct GitLabIntegration {
    token: String,
    project_id: String,
    rev: String,
}

#[derive(Serialize, Deserialize)]
enum State {
    #[serde(rename = "pending")]
    Pending,

    #[serde(rename = "running")]
    Running,

    #[serde(rename = "success")]
    Success,

    #[serde(rename = "failed")]
    Failed,

    #[serde(rename = "canceled")]
    Canceled,
}

#[derive(Serialize, Deserialize)]
struct Query {
    state: State,
    name: Option<String>,
    pipeline_id: Option<usize>,
    target_url: Option<String>,
    description: Option<String>,
}

#[async_trait::async_trait]
impl super::integration::Integration for GitLabIntegration {
    async fn handle_pipeline_start(
        &self,
        state: &common::state::State,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn handle_pipeline_fail(
        &self,
        state: &common::state::State,
        error: Option<String>,
    ) -> Result<(), anyhow::Error> {
        if let Some(error) = error {
            self.set_job_status("pipeline", State::Failed, Some(error))
                .await?;
        }
        Ok(())
    }

    async fn handle_pipeline_done(
        &self,
        state: &common::state::State,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn handle_pipeline_canceled(
        &self,
        state: &common::state::State,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn handle_pipeline_displaced(
        &self,
        state: &common::state::State,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn handle_job_pending(
        &self,
        state: &common::state::State,
        job: &str,
    ) -> Result<(), anyhow::Error> {
        self.set_job_status::<&str>(job, State::Pending, None)
            .await?;
        Ok(())
    }

    async fn handle_job_skipped(
        &self,
        state: &common::state::State,
        job: &str,
    ) -> Result<(), anyhow::Error> {
        self.set_job_status::<&str>(job, State::Success, None)
            .await?;
        Ok(())
    }

    async fn handle_job_canceled(
        &self,
        state: &common::state::State,
        job: &str,
    ) -> Result<(), anyhow::Error> {
        self.set_job_status::<&str>(job, State::Canceled, None)
            .await?;
        Ok(())
    }

    async fn handle_job_progress(
        &self,
        state: &common::state::State,
        job: &str,
        step: usize,
    ) -> Result<(), anyhow::Error> {
        self.set_job_status(job, State::Running, Some(format!("Step {}", step)))
            .await?;
        Ok(())
    }

    async fn handle_job_done(
        &self,
        state: &common::state::State,
        job: &str,
        error: Option<String>,
    ) -> Result<(), anyhow::Error> {
        if let Some(error) = error {
            self.set_job_status(job, State::Failed, Some(format!("Failed: {}", error)))
                .await?;
        } else {
            self.set_job_status::<&str>(job, State::Success, None)
                .await?;
        }
        Ok(())
    }
}

impl GitLabIntegration {
    pub fn from_value(value: serde_json::Value) -> Result<Self, anyhow::Error> {
        let int = serde_json::from_value(value)?;
        Ok(int)
    }

    async fn set_job_status<DS: AsRef<str>>(
        &self,
        name: impl AsRef<str>,
        state: State,
        description: Option<DS>,
    ) -> Result<(), anyhow::Error> {
        let url = format!(
            "https://gitlab.com/api/v4/projects/{}/statuses/{}",
            self.project_id, self.rev
        );
        let query = Query {
            state,
            name: Some(name.as_ref().to_string()),
            pipeline_id: None,
            target_url: None,
            description: description.map(|d| d.as_ref().to_string()),
        };

        let client = reqwest::Client::new();
        let res = client
            .post(url)
            .header("PRIVATE-TOKEN", &self.token)
            .query(&query)
            .send()
            .await?;

        Ok(())
    }
}
