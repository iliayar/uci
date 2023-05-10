use serde::{Deserialize, Serialize};

use std::io::Write;

#[derive(Serialize, Deserialize)]
pub struct TelegramIntegration {
    token: String,
    chat_id: String,

    topic_id: Option<String>,

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
    async fn handle_pipeline_start(
        &self,
        state: &common::state::State,
    ) -> Result<(), anyhow::Error> {
        let text = if let Some(pipeline_id) = self.pipeline_id.as_ref() {
            format!("Starting pipeline {}", pipeline_id)
        } else {
            "Starting pipeline".to_string()
        };

        self.send_message(text).await
    }

    async fn handle_pipeline_fail(
        &self,
        state: &common::state::State,
        error: Option<String>,
    ) -> Result<(), anyhow::Error> {
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

    async fn handle_pipeline_done(
        &self,
        state: &common::state::State,
    ) -> Result<(), anyhow::Error> {
        let text = if let Some(pipeline_id) = self.pipeline_id.as_ref() {
            format!("Pipeline {} finished", pipeline_id)
        } else {
            "Pipeline finished".to_string()
        };

        self.send_message(text).await
    }

    async fn handle_job_pending(
        &self,
        state: &common::state::State,
        job: &str,
    ) -> Result<(), anyhow::Error> {
        if !self.notify_jobs {
            return Ok(());
        }

        let mut buf: Vec<u8> = Vec::new();
        write!(buf, "Job {}", job).ok();

        if let Some(pipeline_id) = self.pipeline_id.as_ref() {
            write!(buf, " in pipeline {}", pipeline_id).ok();
        }

        write!(buf, " pending").ok();

        self.send_message(String::from_utf8_lossy(&buf).to_string())
            .await
    }

    async fn handle_job_skipped(
        &self,
        state: &common::state::State,
        job: &str,
    ) -> Result<(), anyhow::Error> {
        if !self.notify_jobs {
            return Ok(());
        }

        let mut buf: Vec<u8> = Vec::new();
        write!(buf, "Job {}", job).ok();

        if let Some(pipeline_id) = self.pipeline_id.as_ref() {
            write!(buf, " in pipeline {}", pipeline_id).ok();
        }

        write!(buf, " skiped").ok();

        self.send_message(String::from_utf8_lossy(&buf).to_string())
            .await
    }

    async fn handle_job_progress(
        &self,
        state: &common::state::State,
        job: &str,
        step: usize,
    ) -> Result<(), anyhow::Error> {
        if !self.notify_jobs {
            return Ok(());
        }

        let mut buf: Vec<u8> = Vec::new();
        write!(buf, "Job {}", job).ok();

        if let Some(pipeline_id) = self.pipeline_id.as_ref() {
            write!(buf, " in pipeline {}", pipeline_id).ok();
        }

        write!(buf, " executing at step {}", step).ok();

        self.send_message(String::from_utf8_lossy(&buf).to_string())
            .await
    }

    async fn handle_job_done(
        &self,
        state: &common::state::State,
        job: &str,
        error: Option<String>,
    ) -> Result<(), anyhow::Error> {
        if !self.notify_jobs {
            return Ok(());
        }

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

    #[serde(skip_serializing_if = "Option::is_none")]
    message_thread_id: Option<i64>,
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
        let message_thread_id = self
            .topic_id
            .as_ref()
            .map(|id| id.parse::<i64>().ok())
            .flatten();
        let body = SendMessageBody {
            chat_id: self.chat_id.clone(),
            parse_mode: None,
            message_thread_id,
            text,
        };

        self.call("sendMessage", body).await
    }
}
