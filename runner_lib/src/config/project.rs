use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use anyhow::anyhow;
use common::state::State;
use log::*;

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

pub struct EventActions {
    pub run_pipelines: HashSet<String>,
    pub services: HashMap<String, super::ServiceAction>,
}

impl EventActions {
    pub fn is_empty(&self) -> bool {
        self.run_pipelines.is_empty() && self.services.is_empty()
    }
}

impl Project {
    pub async fn load<'a>(state: &State<'a>) -> Result<Project, anyhow::Error> {
        let mut state = state.clone();
        let project_info: &super::ProjectInfo = state.get()?;

        let project_id: String = project_info.id.clone();
        let project_root: PathBuf = project_info.path.clone();

        let params = load_params(project_root.join(PARAMS_CONFIG), &state)
            .await?
            .unwrap_or_default();
        state.set_named("project_params", &params);

        let project_config = project_root.join(PROJECT_CONFIG);
        state.set_named("project_config", &project_config);

        let bind = super::Bind::load(&state)
            .await
            .map_err(|err| anyhow!("Failed to load bind config: {}", err))?;
        let caddy = super::Caddy::load(&state)
            .await
            .map_err(|err| anyhow!("Failed to load caddy config: {}", err))?;

        let services = super::Services::load(&state)
            .await
            .map_err(|err| anyhow!("Failed to load services: {}", err))?;

        let mut state = state.clone();
        state.set(&services);

        let actions = super::Actions::load(&state)
            .await
            .map_err(|err| anyhow!("Failed to load actions: {}", err))?;

        let pipelines = super::Pipelines::load(&state)
            .await
            .map_err(|err| anyhow!("Failed to load pipelines: {}", err))?;

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
        state: &State<'a>,
        pipeline_id: &str,
    ) -> Result<(), anyhow::Error> {
        let pipeline = self
            .pipelines
            .get(pipeline_id)
            .ok_or_else(|| anyhow!("Now such pipeline to run {}", pipeline_id))?;

        self.run_pipeline_impl(Id::Pipeline(pipeline_id), state, pipeline.clone())
            .await?;

        Ok(())
    }

    pub async fn run_service_action<'a>(
        &self,
        state: &State<'a>,
        service_id: impl AsRef<str>,
        action: super::ServiceAction,
    ) -> Result<(), anyhow::Error> {
	let service_id = service_id.as_ref();
        let service = self
            .services
            .get(service_id)
            .ok_or_else(|| anyhow!("Now such service {} to run action on", service_id))?;

        let job = match action {
            super::ServiceAction::Deploy => service.get_deploy_job().ok_or_else(|| {
                anyhow!("Cannot construct deploy config for service {}", service_id)
            })?,
            super::ServiceAction::Logs { follow, tail } => {
                service.get_logs_job(follow, tail).ok_or_else(|| {
                    anyhow!("Cannot construct logs config for service {}", service_id)
                })?
            }
        };

        let mut jobs = HashMap::new();
        jobs.insert(action.to_string(), job);

        let pipeline = common::Pipeline {
            jobs,
            id: format!("service-action@{}", service_id),
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
        state: &State<'a>,
        pipeline: common::Pipeline,
    ) -> Result<(), anyhow::Error> {
        let mut state = state.clone();
        state.set_named("project", &self.id);

        match id {
            Id::Pipeline(id) => info!("Running pipeline {}", id),
            Id::Service(id) => info!("Running service {} action", id),
            Id::Other(id) => info!("Running pipeline for {} action", id),
        };

        if state.get_named::<(), _>("worker").is_ok() {
            let executor: &worker_lib::executor::Executor = state.get()?;
            executor.run_result(&state, pipeline).await?;
        } else {
            let worker_url: Option<String> = state.get_named("worker_url").cloned().ok();
            let worker_url = worker_url.as_ref().ok_or_else(|| {
                anyhow!(
                    "Worker url is not specified in config.
                 Specify it or add '--worker' flag to run pipeline in the same process"
                )
            })?;
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

    pub async fn handle_event<'a>(
        &self,
        state: &State<'a>,
        event: &super::Event,
    ) -> Result<(), anyhow::Error> {
        let EventActions {
            run_pipelines,
            services,
        } = self.actions.get_matched_actions(event).await?;

        let mut pipeline_tasks = Vec::new();
        let mut service_tasks = Vec::new();

        info!("Running pipelines {:?}", run_pipelines);
        for pipeline_id in run_pipelines.iter() {
            if self.pipelines.get(pipeline_id).is_none() {
                warn!("No such pipeline {}, skiping", pipeline_id);
            }
            pipeline_tasks.push(self.run_pipeline(state, pipeline_id))
        }

        info!("Running service actions {:?}", services);
        for (service, action) in services.iter() {
            if self.services.get(service).is_none() {
                warn!("No such service {}, skiping", service);
            }
            service_tasks.push(self.run_service_action(state, service.to_string(), action.clone()));
        }

        futures::future::try_join_all(pipeline_tasks).await?;
        futures::future::try_join_all(service_tasks).await?;

        Ok(())
    }
}

pub async fn load_params<'a>(
    params_file: PathBuf,
    state: &State<'a>,
) -> Result<Option<HashMap<String, String>>, anyhow::Error> {
    if !params_file.exists() {
        return Ok(None);
    }

    let env: String = state.get_named("env").cloned()?;
    let content = tokio::fs::read_to_string(params_file).await?;
    let params: HashMap<String, HashMap<String, String>> = serde_yaml::from_str(&content)?;

    let default_params = params.get("__default__").cloned().unwrap_or_default();
    let env_params = params.get(&env).cloned().unwrap_or_default();

    let mut result = HashMap::new();

    for (key, value) in env_params.into_iter() {
        result.insert(key, super::utils::substitute_vars(state, value)?);
    }

    for (key, value) in default_params.into_iter() {
        if let std::collections::hash_map::Entry::Vacant(e) = result.entry(key) {
            e.insert(super::utils::substitute_vars(state, value)?);
        }
    }

    Ok(Some(result))
}

enum Id<'a> {
    Pipeline(&'a str),
    Service(&'a str),
    Other(&'a str),
}
