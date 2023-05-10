use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct GitHubIntegration {
    token: String,
    repo: String,
    rev: String,
}

#[derive(Serialize, Deserialize)]
enum State {
    #[serde(rename = "pending")]
    Pending,

    #[serde(rename = "success")]
    Success,

    #[serde(rename = "failure")]
    Failure,

    #[serde(rename = "Error")]
    Error,
}

#[async_trait::async_trait]
impl super::integration::Integration for GitHubIntegration {
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
            self.set_job_status("pipeline", State::Failure, Some(error))
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

    async fn handle_job_progress(
        &self,
        state: &common::state::State,
        job: &str,
        step: usize,
    ) -> Result<(), anyhow::Error> {
        self.set_job_status(job, State::Pending, Some(format!("Step {}", step)))
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
            self.set_job_status(job, State::Failure, Some(format!("Failed: {}", error)))
                .await?;
        } else {
            self.set_job_status::<&str>(job, State::Success, None)
                .await?;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct Body {
    state: State,

    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    target_url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

impl GitHubIntegration {
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
            "https://api.github.com/repos/{}/statuses/{}",
            self.repo, self.rev
        );
        let body = Body {
            state,
            context: Some(name.as_ref().to_string()),
            target_url: None,
            description: description.map(|d| d.as_ref().to_string()),
        };

        let client = reqwest::Client::builder();
        let res = client
            .user_agent("uCI")
            .build()?
            .post(url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await?;

        res.error_for_status()?;

        Ok(())
    }
}
