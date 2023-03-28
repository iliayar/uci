use std::path::PathBuf;

use anyhow::anyhow;
use log::*;

#[derive(Debug)]
pub struct Project {
    pub path: PathBuf,
    pub actions: super::Actions,
    pub pipelines: super::Pipelines,
}

impl Project {
    pub async fn load(project_root: PathBuf) -> Result<Project, super::LoadConfigError> {
        Ok(Project {
            path: project_root.clone(),
            actions: super::Actions::load(project_root.clone()).await?,
            pipelines: super::Pipelines::load(project_root).await?,
        })
    }

    pub async fn run_pipeline(
        &self,
        config: &super::ServiceConfig,
        pipeline_id: &str,
    ) -> Result<(), super::ExecutionError> {
        let pipeline = self
            .pipelines
            .get(pipeline_id)
            .ok_or(anyhow!("Now such pipeline to run {}", pipeline_id))?;

        info!("Running pipeline {}", pipeline_id);
        let response = reqwest::Client::new()
            .post(&format!("{}/run", config.worker_url))
            .json(pipeline)
            .send()
            .await?;

        response.error_for_status()?;

        info!("Pipeline {} started", pipeline_id);

        Ok(())
    }
}
