use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

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
    pub caddy: Option<super::Caddy>,
    pub params: HashMap<String, String>,
}

const PROJECT_CONFIG: &str = "project.yaml";
const PARAMS_CONFIG: &str = "params.yaml";

pub struct ProjectMatchedActions {
    pub reload_config: bool,
    pub reload_project: bool,
    pub run_pipelines: HashSet<String>,
    pub services: HashMap<String, super::ServiceAction>,
}

impl Project {
    pub async fn load<'a>(
        context: &super::LoadContext<'a>,
    ) -> Result<Project, super::LoadConfigError> {
        let mut context = context.clone();

        let params = load_params(context.project_root()?.join(PARAMS_CONFIG), &context)
            .await?
            .unwrap_or_default();
        context.set_params(&params);

        let project_config = context.project_root()?.join(PROJECT_CONFIG);
        context.set_project_config(&project_config);

        let bind = super::Bind::load(&context).await?;
        let caddy = super::Caddy::load(&context).await?;

        if bind.is_some() {
            context.set_extra(String::from("dns"), "");
        }

        if caddy.is_some() {
            context.set_extra(String::from("caddy"), "");
        }

        let services = super::Services::load(&context).await?;

        let actions = super::Actions::load(&context).await?;

        context.set_networks(&services.networks);
        context.set_volumes(&services.volumes);
        let pipelines = super::Pipelines::load(&context).await?;

        Ok(Project {
            id: context.project_id()?.to_string(),
            path: context.project_root()?.clone(),
            params,
            actions,
            services,
            pipelines,
            bind,
            caddy,
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

    pub async fn run_matched_action(
        &self,
        config: &super::ServiceConfig,
        worker_context: Option<worker_lib::context::Context>,
        ProjectMatchedActions {
            reload_config,
            run_pipelines,
            services,
	    reload_project,
        }: ProjectMatchedActions,
    ) -> Result<(), super::ExecutionError> {
        let mut pipeline_tasks = Vec::new();
        let mut service_tasks = Vec::new();

        info!("Running pipelines {:?}", run_pipelines);
        for pipeline_id in run_pipelines.iter() {
            pipeline_tasks.push(self.run_pipeline(config, worker_context.clone(), pipeline_id))
        }

        info!("Running service actions {:?}", services);
        for (service, action) in services.iter() {
            service_tasks.push(self.run_service_action(
                config,
                worker_context.clone(),
                service.to_string(),
                action.clone(),
            ));
        }

        futures::future::try_join_all(pipeline_tasks).await?;
        futures::future::try_join_all(service_tasks).await?;

        Ok(())
    }

    pub async fn get_matched_actions(
        &self,
        event: &super::Event,
    ) -> Result<ProjectMatchedActions, super::ExecutionError> {
        Ok(self.actions.get_matched_actions(event).await?)
    }
}

pub async fn load_params<'a>(
    params_file: PathBuf,
    context: &super::LoadContext<'a>,
) -> Result<Option<HashMap<String, String>>, super::LoadConfigError> {
    if !params_file.exists() {
        return Ok(None);
    }

    let vars: common::vars::Vars = context.into();

    let env = context.env()?;
    let content = tokio::fs::read_to_string(params_file).await?;
    let params: HashMap<String, HashMap<String, String>> = serde_yaml::from_str(&content)?;

    let default_params = params.get("__default__").cloned().unwrap_or_default();
    let env_params = params.get(env).cloned().unwrap_or_default();

    let mut result = HashMap::new();

    for (key, value) in env_params.into_iter() {
        result.insert(key, vars.eval(&value)?);
    }

    for (key, value) in default_params.into_iter() {
        if !result.contains_key(&key) {
            result.insert(key, vars.eval(&value)?);
        }
    }

    Ok(Some(result))
}

enum Id<'a> {
    Pipeline(&'a str),
    Service(&'a str),
    Other(&'a str),
}
