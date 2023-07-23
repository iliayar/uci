use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct GitHubIntegration {
    token: String,
    repo: String,
    rev: String,
    jobs_to_report: Option<Vec<String>>,
    ui_url: Option<String>,
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
            self.set_job_status(state, "pipeline", State::Failure, Some(error))
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
        if self.should_skip_job(job) {
            return Ok(());
        }

        self.set_job_status::<&str>(state, job, State::Pending, None)
            .await?;
        Ok(())
    }

    async fn handle_job_skipped(
        &self,
        state: &common::state::State,
        job: &str,
    ) -> Result<(), anyhow::Error> {
        if self.should_skip_job(job) {
            return Ok(());
        }

        self.set_job_status::<&str>(state, job, State::Success, None)
            .await?;
        Ok(())
    }

    async fn handle_job_canceled(
        &self,
        state: &common::state::State,
        job: &str,
    ) -> Result<(), anyhow::Error> {
        if self.should_skip_job(job) {
            return Ok(());
        }

        self.set_job_status::<&str>(state, job, State::Error, None)
            .await?;
        Ok(())
    }

    async fn handle_job_progress(
        &self,
        state: &common::state::State,
        job: &str,
        step: usize,
    ) -> Result<(), anyhow::Error> {
        if self.should_skip_job(job) {
            return Ok(());
        }

        self.set_job_status(state, job, State::Pending, Some(format!("Step {}", step)))
            .await?;
        Ok(())
    }

    async fn handle_job_done(
        &self,
        state: &common::state::State,
        job: &str,
        error: Option<String>,
    ) -> Result<(), anyhow::Error> {
        if self.should_skip_job(job) {
            return Ok(());
        }

        if let Some(error) = error {
            self.set_job_status(
                state,
                job,
                State::Failure,
                Some(format!("Failed: {}", error)),
            )
            .await?;
        } else {
            self.set_job_status::<&str>(state, job, State::Success, None)
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
    fn should_skip_job(&self, job: &str) -> bool {
        if let Some(jobs_to_report) = self.jobs_to_report.as_ref() {
            // FIXME: meh
            !jobs_to_report.contains(&job.to_string())
        } else {
            false
        }
    }

    pub fn from_value(value: serde_json::Value) -> Result<Self, anyhow::Error> {
        let int = serde_json::from_value(value)?;
        Ok(int)
    }

    async fn set_job_status<'a, DS: AsRef<str>>(
        &self,
        state: &common::state::State<'a>,
        job: impl AsRef<str>,
        job_state: State,
        description: Option<DS>,
    ) -> Result<(), anyhow::Error> {
        let url = format!(
            "https://api.github.com/repos/{}/statuses/{}",
            self.repo, self.rev
        );

        let project: String = state.get_named("project").cloned()?;
        let pipeline_run: &crate::executor::PipelineRun = state.get()?;


	let name = format!("{}/{}", pipeline_run.pipeline_id, job.as_ref());

        let target_url = self
            .ui_url
            .as_ref()
            .map(|url| -> Result<String, anyhow::Error> {
                Ok(format!(
                    "{}/projects/{}/runs/{}/{}",
                    url, project, pipeline_run.id, pipeline_run.pipeline_id
                ))
            });
        let target_url = if let Some(target_url) = target_url {
            Some(target_url?)
        } else {
            None
        };

        let body = Body {
            state: job_state,
            context: Some(name),
            description: description.map(|d| d.as_ref().to_string()),
            target_url,
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
