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

impl ProjectMatchedActions {
    pub fn is_empty(&self) -> bool {
        !self.reload_config
            && !self.reload_project
            && self.run_pipelines.is_empty()
            && self.services.is_empty()
    }
}

impl Project {
    pub async fn load<'a>(context: &super::State<'a>) -> Result<Project, super::LoadConfigError> {
        let mut context = context.clone();

        let project_id: String = context.get_named("project_id").cloned()?;
        let project_root: PathBuf = context.get_named("project_root").cloned()?;

        let params = load_params(project_root.join(PARAMS_CONFIG), &context)
            .await?
            .unwrap_or_default();
        // let params_binding = super::binding::Params { value: &params };
        // context.set(&params_binding);

        let project_config = project_root.join(PROJECT_CONFIG);
        context.set_named("project_config", &project_config);

        let bind = super::Bind::load(&context).await?;
        let caddy = super::Caddy::load(&context).await?;

        let services = super::Services::load(&context).await?;

        let mut context = context.clone();
	context.set(&services);

        let actions = super::Actions::load(&context).await?;

        let pipelines = super::Pipelines::load(&context).await?;

        Ok(Project {
            id: project_id,
            path: project_root,
            params,
            actions,
            services,
            pipelines,
            bind,
            caddy,
        })
    }

    pub async fn run_pipeline<'a>(
        &self,
        state: &super::State<'a>,
        pipeline_id: &str,
    ) -> Result<(), super::ExecutionError> {
        let pipeline = self
            .pipelines
            .get(pipeline_id)
            .ok_or(anyhow!("Now such pipeline to run {}", pipeline_id))?;

        self.run_pipeline_impl(Id::Pipeline(pipeline_id), state, pipeline.clone())
            .await?;

        Ok(())
    }

    pub async fn run_service_action<'a>(
        &self,
        state: &super::State<'a>,
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

        self.run_pipeline_impl(Id::Service(&service_id), state, pipeline)
            .await?;

        Ok(())
    }

    async fn run_pipeline_impl<'a>(
        &self,
        id: Id<'a>,
        state: &super::State<'a>,
        pipeline: common::Pipeline,
    ) -> Result<(), super::ExecutionError> {
        let worker_context: &Option<worker_lib::context::Context> = state.get()?;
        match id {
            Id::Pipeline(id) => info!("Running pipeline {}", id),
            Id::Service(id) => info!("Running service {} action", id),
            Id::Other(id) => info!("Running pipeline for {} action", id),
        };

        if let Some(worker_context) = worker_context.clone() {
            let executor = worker_lib::executor::Executor::new(worker_context)?;
            executor.run_result(pipeline).await?;
        } else {
            let worker_url: Option<String> = state.get_named("worker_url").cloned().ok();
            let worker_url = worker_url.as_ref().ok_or(anyhow!(
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

    pub async fn run_matched_action<'a>(
        &self,
        state: &super::State<'a>,
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
            if let None = self.pipelines.get(pipeline_id) {
                warn!("No such pipeline {}, skiping", pipeline_id);
            }
            pipeline_tasks.push(self.run_pipeline(state, pipeline_id))
        }

        info!("Running service actions {:?}", services);
        for (service, action) in services.iter() {
            if let None = self.services.get(service) {
                warn!("No such service {}, skiping", service);
            }
            service_tasks.push(self.run_service_action(state, service.to_string(), action.clone()));
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
    context: &super::State<'a>,
) -> Result<Option<HashMap<String, String>>, super::LoadConfigError> {
    if !params_file.exists() {
        return Ok(None);
    }

    let vars: common::vars::Vars = context.into();

    let env: String = context.get_named("env").cloned()?;
    let content = tokio::fs::read_to_string(params_file).await?;
    let params: HashMap<String, HashMap<String, String>> = serde_yaml::from_str(&content)?;

    let default_params = params.get("__default__").cloned().unwrap_or_default();
    let env_params = params.get(&env).cloned().unwrap_or_default();

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
