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

        let steps = match action {
            super::ServiceAction::Deploy => {
                let build_config = service.get_build_config(&self, config).ok_or(anyhow!(
                    "Cannot construct build config for service {}",
                    service_id
                ))?;
                let run_config = service.get_run_config(&self, config).ok_or(anyhow!(
                    "Cannot construct run config for service {}",
                    service_id
                ))?;
                let stop_config = service.get_stop_config(&self).ok_or(anyhow!(
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

        let mut jobs = HashMap::new();
        jobs.insert(
            String::from("deploy"),
            common::Job {
                needs: Vec::new(),
                steps,
            },
        );

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
        let pipeline = self.prepare_pipeline(config, pipeline.clone()).await?;

        match id {
            Id::Pipeline(id) => info!("Running pipeline {}", id),
            Id::Service(id) => info!("Running service {} action", id),
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
        }

        Ok(())
    }

    async fn prepare_pipeline(
        &self,
        config: &super::ServiceConfig,
        pipeline: common::Pipeline,
    ) -> Result<common::Pipeline, super::ExecutionError> {
        let jobs = pipeline
            .jobs
            .into_iter()
            .map(|(k, v)| (k, self.prepare_job(config, v)))
            .collect();
        let links = super::utils::prepare_links(&self.id, config, &pipeline.links);

        let networks = self
            .services
            .networks
            .iter()
            .map(|(k, v)| super::get_resource_name(&self, k, v.global))
            .collect();

        let volumes = self
            .services
            .volumes
            .iter()
            .map(|(k, v)| super::get_resource_name(&self, k, v.global))
            .collect();

        Ok(common::Pipeline {
            jobs,
            links,
            networks,
            volumes,
        })
    }

    fn prepare_job(&self, config: &super::ServiceConfig, job: common::Job) -> common::Job {
        let steps = job
            .steps
            .into_iter()
            .map(|step| self.prepare_step(config, step))
            .collect();

        common::Job {
            steps,
            needs: job.needs,
        }
    }

    fn prepare_step(&self, config: &super::ServiceConfig, step: common::Step) -> common::Step {
        match step {
            common::Step::RunShell(shell) => {
                let volumes = super::utils::prepare_links(&self.id, config, &shell.volumes);
                let volumes = volumes
                    .into_iter()
                    .map(|(k, v)| (self.services.get_volume_name(&self, &v), k))
                    .collect();
                let networks = shell
                    .networks
                    .iter()
                    .map(|name| self.services.get_network_name(&self, name))
                    .collect();

                common::Step::RunShell(common::RunShellConfig {
                    volumes,
                    networks,
                    ..shell
                })
            }
            step => step,
        }
    }
}

enum Id<'a> {
    Pipeline(&'a str),
    Service(&'a str),
}
