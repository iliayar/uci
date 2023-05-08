use serde::{Deserialize, Serialize};

use std::io::Write;

#[derive(Serialize, Deserialize)]
pub struct TelegramIntegration {
    token: String,
    chat_id: String,

    #[serde(default = "default_notify_jobs")]
    notify_jobs: bool,

    pipeline_id: Option<String>,
}

fn default_notify_jobs() -> bool {
    false
}

impl TelegramIntegration {
    pub fn from_value(value: serde_json::Value) -> Result<Self, anyhow::Error> {
        Ok(serde_json::from_value(value)?)
    }
}

#[async_trait::async_trait]
impl super::integration::Integration for TelegramIntegration {
    async fn handle_pipeline_start(&self) -> Result<(), anyhow::Error> {
        let text = if let Some(pipeline_id) = self.pipeline_id.as_ref() {
            format!("Starting pipeline {}", pipeline_id)
        } else {
            "Starting pipeline".to_string()
        };

        self.send_message(text).await
    }

    async fn handle_pipeline_fail(&self, error: Option<String>) -> Result<(), anyhow::Error> {
        let mut buf: Vec<u8> = Vec::new();

        if let Some(pipeline_id) = self.pipeline_id.as_ref() {
            write!(buf, "Pipeline {}", pipeline_id).ok();
        } else {
            write!(buf, "Pipeline").ok();
        }

        write!(buf, " failed").ok();

        if let Some(error) = error {
            write!(buf, " with error: {}", error).ok();
        }

        self.send_message(String::from_utf8_lossy(&buf).to_string())
            .await
    }

    async fn handle_pipeline_done(&self) -> Result<(), anyhow::Error> {
        let text = if let Some(pipeline_id) = self.pipeline_id.as_ref() {
            format!("Pipeline {} finished", pipeline_id)
        } else {
            "Pipeline finished".to_string()
        };

        self.send_message(text).await
    }

    async fn handle_job_pending(&self, job: &str) -> Result<(), anyhow::Error> {
        let mut buf: Vec<u8> = Vec::new();
        write!(buf, "Job {}", job).ok();

        if let Some(pipeline_id) = self.pipeline_id.as_ref() {
            write!(buf, " in pipeline {}", pipeline_id).ok();
        }

        write!(buf, " pending").ok();

        self.send_message(String::from_utf8_lossy(&buf).to_string())
            .await
    }

    async fn handle_job_progress(&self, job: &str, step: usize) -> Result<(), anyhow::Error> {
        let mut buf: Vec<u8> = Vec::new();
        write!(buf, "Job {}", job).ok();

        if let Some(pipeline_id) = self.pipeline_id.as_ref() {
            write!(buf, " in pipeline {}", pipeline_id).ok();
        }

        write!(buf, " executing at step {}", step).ok();

        self.send_message(String::from_utf8_lossy(&buf).to_string())
            .await
    }

    async fn handle_job_done(&self, job: &str, error: Option<String>) -> Result<(), anyhow::Error> {
        let mut buf: Vec<u8> = Vec::new();
        write!(buf, "Job {}", job).ok();

        if let Some(pipeline_id) = self.pipeline_id.as_ref() {
            write!(buf, " in pipeline {}", pipeline_id).ok();
        }

        if let Some(error) = error {
            write!(buf, " failed with error {}", error).ok();
        } else {
            write!(buf, " done").ok();
        }

        self.send_message(String::from_utf8_lossy(&buf).to_string())
            .await
    }
}

#[derive(Serialize, Deserialize)]
enum ParseMode {
    #[serde(rename = "MarkdownV2")]
    MarkdownV2,

    #[serde(rename = "HTML")]
    Html,
}

#[derive(Serialize, Deserialize)]
struct SendMessageBody {
    chat_id: String,
    text: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    parse_mode: Option<ParseMode>,
}

impl TelegramIntegration {
    async fn call<T: Serialize>(
        &self,
        method: impl AsRef<str>,
        body: T,
    ) -> Result<(), anyhow::Error> {
        let url = format!(
            "https://api.telegram.org/bot{}/{}",
            self.token,
            method.as_ref(),
        );
        let client = reqwest::Client::new();
        let res = client.post(url).json(&body).send().await?;

	res.error_for_status()?;

        Ok(())
    }

    async fn send_message(&self, text: String) -> Result<(), anyhow::Error> {
        let body = SendMessageBody {
            chat_id: self.chat_id.clone(),
            parse_mode: None,
            text,
        };

        self.call("sendMessage", body).await
    }
}
