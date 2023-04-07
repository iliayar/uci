use std::{collections::HashMap, path::PathBuf};

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
    pub bind: Option<super::Bind>,
}

const PROJECT_CONFIG: &str = "project.yaml";

impl Project {
    pub async fn load<'a>(
        context: &super::LoadContext<'a>,
    ) -> Result<Project, super::LoadConfigError> {
        let mut context = context.clone();

        let project_config = context.project_root()?.join(PROJECT_CONFIG);
        context.set_project_config(&project_config);

        let bind = super::Bind::load(&context).await?;

	context.set_extra(String::from("dns"), "");
        let services = super::Services::load(&context).await?;

        let actions = super::Actions::load(&context).await?;

        context.set_networks(&services.networks);
        context.set_volumes(&services.volumes);
        let pipelines = super::Pipelines::load(&context).await?;

        Ok(Project {
            id: context.project_id()?.to_string(),
            path: context.project_root()?.clone(),
            actions,
            services,
            pipelines,
            bind,
        })
    }

    pub async fn autorun(
        &self,
        config: &super::ServiceConfig,
        worker_context: Option<worker_lib::context::Context>,
    ) -> Result<(), super::ExecutionError> {
        let jobs = self.services.autorun().await?;

        let pipeline = common::Pipeline {
            jobs,
            links: Default::default(),
            networks: Default::default(),
            volumes: Default::default(),
        };

        self.run_pipeline_impl(
            Id::Other(&format!("autorun_{}", self.id)),
            config,
            worker_context,
            pipeline,
        )
        .await?;

        Ok(())
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

        self.run_pipeline_impl(
            Id::Pipeline(pipeline_id),
            config,
            worker_context,
            pipeline.clone(),
        )
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

        let job = match action {
            super::ServiceAction::Deploy => service.get_deploy_job().ok_or(anyhow!(
                "Cannot construct deploy config for service {}",
                service_id
            ))?,
        };

        let mut jobs = HashMap::new();
        jobs.insert(String::from("deploy"), job);

        let pipeline = common::Pipeline {
            jobs,
            links: Default::default(),
            networks: Default::default(),
            volumes: Default::default(),
        };

        self.run_pipeline_impl(Id::Service(&service_id), config, worker_context, pipeline)
            .await?;

        Ok(())
    }

    async fn run_pipeline_impl<'a>(
        &self,
        id: Id<'a>,
        config: &super::ServiceConfig,
        worker_context: Option<worker_lib::context::Context>,
        pipeline: common::Pipeline,
    ) -> Result<(), super::ExecutionError> {
        match id {
            Id::Pipeline(id) => info!("Running pipeline {}", id),
            Id::Service(id) => info!("Running service {} action", id),
            Id::Other(id) => info!("Running pipeline for {} action", id),
        };

        if let Some(worker_context) = worker_context {
            let executor = worker_lib::executor::Executor::new(worker_context)?;
            tokio::spawn(executor.run(pipeline));
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
            Id::Other(id) => info!("Pipeline for {} started", id),
        }

        Ok(())
    }
}

enum Id<'a> {
    Pipeline(&'a str),
    Service(&'a str),
    Other(&'a str),
}
