use std::path::PathBuf;

use anyhow::anyhow;
use log::*;

use worker_lib;

#[derive(Debug)]
pub struct Project {
    pub id: String,
    pub path: PathBuf,
    pub actions: super::Actions,
    pub pipelines: super::Pipelines,
    pub services: super::Services,
}

impl Project {
    pub async fn load(
        project_id: String,
        project_root: PathBuf,
    ) -> Result<Project, super::LoadConfigError> {
        Ok(Project {
            id: project_id,
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

        self.run_pipeline_impl(Id::Pipeline(pipeline_id), config, worker_context, pipeline)
            .await?;

        Ok(())
    }

    pub async fn run_service_action(
        &self,
        config: &super::ServiceConfig,
        worker_context: Option<worker_lib::context::Context>,
        service_id: String,
        action: super::ServiceAction,
    ) -> Result<(), super::ExecutionError> {
        let service = self
            .services
            .get(&service_id)
            .ok_or(anyhow!("Now such service {} to run action on", service_id))?;

        let steps = match action {
            super::ServiceAction::Deploy => {
                let build_config = service.get_build_config(&self.id, config).ok_or(anyhow!(
                    "Cannot construct build config for service {}",
                    service_id
                ))?;
                let run_config = service.get_run_config(&self.id).ok_or(anyhow!(
                    "Cannot construct run config for service {}",
                    service_id
                ))?;
                let stop_config = service.get_stop_config(&self.id).ok_or(anyhow!(
                    "Cannot construct stop config for service {}",
                    service_id
                ))?;

                vec![common::Step::Deploy(common::DeployConfig {
                    stop_config,
                    build_config,
                    run_config,
                })]
            }
        };

        let pipeline = common::Pipeline { steps };

        self.run_pipeline_impl(Id::Pipeline(&service_id), config, worker_context, &pipeline)
            .await?;

        Ok(())
    }

    async fn run_pipeline_impl<'a>(
        &self,
        id: Id<'a>,
        config: &super::ServiceConfig,
        worker_context: Option<worker_lib::context::Context>,
        pipeline: &common::Pipeline,
    ) -> Result<(), super::ExecutionError> {
        match id {
            Id::Pipeline(id) => info!("Running pipeline {}", id),
            Id::Service(id) => info!("Running service {} action", id),
        };

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
                .json(&pipeline)
                .send()
                .await?;

            response.error_for_status()?;
        }

        match id {
            Id::Pipeline(id) => info!("Pipeline {} started", id),
            Id::Service(id) => info!("Service {} action started", id),
        }

        Ok(())
    }
}

enum Id<'a> {
    Pipeline(&'a str),
    Service(&'a str),
}
