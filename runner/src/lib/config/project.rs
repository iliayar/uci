use std::path::PathBuf;

use anyhow::anyhow;
use log::*;

use worker_lib;

#[derive(Debug)]
pub struct Project {
    pub path: PathBuf,
    pub actions: super::Actions,
    pub pipelines: super::Pipelines,
    pub services: super::Services,
}

impl Project {
    pub async fn load(project_root: PathBuf) -> Result<Project, super::LoadConfigError> {
        Ok(Project {
            path: project_root.clone(),
            actions: super::Actions::load(project_root.clone()).await?,
            pipelines: super::Pipelines::load(project_root.clone()).await?,
            services: super::Services::load(project_root.clone()).await?,
        })
    }

    pub async fn run_pipeline(
        &self,
        config: &super::ServiceConfig,
        worker_context: Option<worker_lib::context::Context>,
        pipeline_id: &str,
    ) -> Result<(), super::ExecutionError> {
        let pipeline = self
            .pipelines
            .get(pipeline_id)
            .ok_or(anyhow!("Now such pipeline to run {}", pipeline_id))?;

        info!("Running pipeline {}", pipeline_id);
        if let Some(worker_context) = worker_context {
            let executor = worker_lib::executor::Executor::new(worker_context)?;
            tokio::spawn(executor.run(pipeline.clone()));
        } else {
            let worker_url = config.worker_url.as_ref().ok_or(anyhow!(
                "Worker url is not specified in config.
                 Specify it or add '--worker' flag to run pipeline in the same process"
            ))?;
            let response = reqwest::Client::new()
                .post(&format!("{}/run", worker_url))
                .json(pipeline)
                .send()
                .await?;

            response.error_for_status()?;
        }

        info!("Pipeline {} started", pipeline_id);

        Ok(())
    }

    pub async fn run_service_action(
        &self,
        service: String,
        action: super::ServiceAction,
    ) -> Result<(), super::ExecutionError> {
        todo!()
    }
}
