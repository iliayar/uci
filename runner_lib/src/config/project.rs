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
    pub actions: super::Actions,
    pub pipelines: super::Pipelines,
    pub services: super::Services,
    pub bind: Vec<super::Bind>,
    pub caddy: Vec<super::Caddy>,

    // TODO: Allow common::vars::Value here
    pub params: HashMap<String, String>,
}

pub struct CurrentProject {
    pub path: PathBuf,
}

const PROJECT_CONFIG: &str = "project.yaml";
const PARAMS_CONFIG: &str = "params.yaml";

pub struct EventActions {
    pub run_pipelines: HashSet<String>,
    pub services: HashMap<String, super::ServiceAction>,
    pub params: HashMap<String, common::vars::Value>,
}

impl EventActions {
    pub fn is_empty(&self) -> bool {
        self.run_pipelines.is_empty() && self.services.is_empty()
    }
}

impl Project {
    pub fn merge(self, other: Project) -> Result<Project, anyhow::Error> {
        assert!(self.id == other.id);
        let id = self.id;

        let actions = self.actions.merge(other.actions)?;
        let pipelines = self.pipelines.merge(other.pipelines)?;
        let services = self.services.merge(other.services)?;

        let caddy = self
            .caddy
            .into_iter()
            .chain(other.caddy.into_iter())
            .collect();

        let bind = self
            .bind
            .into_iter()
            .chain(other.bind.into_iter())
            .collect();

        let mut params = HashMap::new();
        for (id, value) in self.params.into_iter().chain(other.params.into_iter()) {
            if params.contains_key(&id) {
                return Err(anyhow!("Param {} duplicates", id));
            }
            params.insert(id, value);
        }

        Ok(Project {
            id,
            actions,
            pipelines,
            services,
            caddy,
            bind,
            params,
        })
    }

    pub async fn load<'a>(state: &State<'a>) -> Result<Project, anyhow::Error> {
        let project_info: &super::ProjectInfo = state.get()?;
        let project_id: String = project_info.id.clone();

        let mut project: Option<Project> = None;

        // NOTE: Maybe merge not a whole project, but parts
        for project_root in project_info.path.iter() {
            let mut state = state.clone();

	    let current_project = CurrentProject {
		path: project_root.clone(),
	    };
	    state.set(&current_project);

            let params = load_params(project_root.join(PARAMS_CONFIG), &state)
                .await?
                .unwrap_or_default();
            state.set_named("project_params", &params);

            let project_config = project_root.join(PROJECT_CONFIG);
            state.set_named("project_config", &project_config);

            let bind = super::Bind::load(&state)
                .await
                .map_err(|err| anyhow!("Failed to load bind config: {}", err))?
                .into_iter()
                .collect();
            let caddy = super::Caddy::load(&state)
                .await
                .map_err(|err| anyhow!("Failed to load caddy config: {}", err))?
                .into_iter()
                .collect();

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

            let new_project = Project {
                id: project_id.clone(),
                params,
                actions,
                services,
                pipelines,
                bind,
                caddy,
            };

            project = if let Some(project) = project.take() {
                Some(project.merge(new_project)?)
            } else {
                Some(new_project)
            };
        }

        Ok(project.ok_or_else(|| anyhow!("Must be at least one project"))?)
    }

    pub async fn run_pipeline<'a>(
        &self,
        state: &State<'a>,
        pipeline_id: &str,
    ) -> Result<(), anyhow::Error> {
        let mut state = state.clone();
        state.set_named("project_params", &self.params);
        state.set(&self.services);

        let pipeline = self.pipelines.get(&state, pipeline_id).await?;

        self.run_pipeline_impl(&state, pipeline.clone()).await?;

        Ok(())
    }

    pub async fn run_service_actions<'a>(
        &self,
        state: &State<'a>,
        actions: HashMap<String, super::ServiceAction>,
    ) -> Result<(), anyhow::Error> {
        if actions.is_empty() {
            return Ok(());
        }

        let mut jobs = HashMap::new();
        for (service_id, action) in actions.into_iter() {
            let service_id = service_id.as_ref();
            let service = self
                .services
                .get(service_id)
                .ok_or_else(|| anyhow!("Now such service {} to run action on", service_id))?;

            let job = match action {
                super::ServiceAction::Deploy => {
                    service.get_restart_job(/* build */ true).ok_or_else(|| {
                        anyhow!("Cannot construct deploy config for service {}", service_id)
                    })?
                }
                super::ServiceAction::Start { build } => {
                    service.get_start_job(build).ok_or_else(|| {
                        anyhow!("Cannot construct start config for service {}", service_id)
                    })?
                }
                super::ServiceAction::Stop => service.get_stop_job().ok_or_else(|| {
                    anyhow!("Cannot construct stop config for service {}", service_id)
                })?,
                super::ServiceAction::Restart { build } => {
                    service.get_restart_job(build).ok_or_else(|| {
                        anyhow!("Cannot construct restart config for service {}", service_id)
                    })?
                }
                super::ServiceAction::Logs { follow, tail } => {
                    service.get_logs_job(follow, tail).ok_or_else(|| {
                        anyhow!("Cannot construct logs config for service {}", service_id)
                    })?
                }
            };

            jobs.insert(format!("{}@{}", action.to_string(), service_id), job);
        }

        let stage = common::Stage {
            overlap_strategy: common::OverlapStrategy::Wait,
            repos: None,
        };

        let pipeline = common::Pipeline {
            jobs,
            stages: HashMap::from_iter([(worker_lib::executor::DEFEAULT_STAGE.to_string(), stage)]),
            id: "service-action".to_string(),
            links: Default::default(),
            networks: Default::default(),
            volumes: Default::default(),
            integrations: Default::default(),
        };

        self.run_pipeline_impl(state, pipeline).await?;

        Ok(())
    }

    async fn run_pipeline_impl<'a>(
        &self,
        state: &State<'a>,
        pipeline: common::Pipeline,
    ) -> Result<(), anyhow::Error> {
        let pipeline_id = pipeline.id.clone();

        let mut state = state.clone();
        state.set_named("project", &self.id);

        info!("Running pipeline {}", pipeline_id);

        let pinfo: &super::ProjectInfo = state.get()?;
        let repos_list = worker_lib::executor::ReposList {
            project: self.id.clone(),
            repos: pinfo.repos.list_repos(),
        };
        state.set(&repos_list);

        let executor: &worker_lib::executor::Executor = state.get()?;
        executor.run_result(&state, pipeline).await?;

        info!("Pipeline {} started", pipeline_id);

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
            params,
        } = self.actions.get_matched_actions(event).await?;

        let mut state = state.clone();
        state.set_named("action_params", &params);

        let mut pipeline_tasks = Vec::new();

        info!("Running pipelines {:?}", run_pipelines);
        for pipeline_id in run_pipelines.iter() {
            pipeline_tasks.push(self.run_pipeline(&state, pipeline_id))
        }

        let services_fut = self.run_service_actions(&state, services);
        let pipelines_fut = futures::future::try_join_all(pipeline_tasks);

        tokio::try_join!(pipelines_fut, services_fut)?;

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
