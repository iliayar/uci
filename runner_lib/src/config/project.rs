use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use anyhow::{anyhow, Result};
use common::state::State;
use log::*;

use crate::config;

#[derive(Debug)]
pub struct Project {
    pub id: String,
    pub actions: config::actions::Actions,
    pub pipelines: config::pipelines::Pipelines,
    pub services: config::services::Services,
    pub bind: Vec<config::bind::Bind>,
    pub caddy: Vec<config::caddy::Caddy>,
    pub params: dynconf::Value,
}

pub struct CurrentProject {
    pub path: PathBuf,
}

pub struct EventActions {
    pub run_pipelines: HashSet<String>,
    pub services: HashMap<String, config::actions::ServiceAction>,
    pub params: dynconf::Value,
}

impl EventActions {
    pub fn is_empty(&self) -> bool {
        self.run_pipelines.is_empty() && self.services.is_empty()
    }
}

impl Project {
    pub fn merge(self, other: Project) -> Result<Project> {
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

        let params = self.params.merge(other.params)?;

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

    pub async fn run_pipeline<'a>(
        &self,
        state: &State<'a>,
        pipeline_id: &str,
    ) -> Result<(), anyhow::Error> {
	let project_params = ProjectParams(self.params.clone());
        let mut state = state.clone();
        state.set(&self.services);
	state.set(&project_params);

        let pipeline = self.pipelines.get(&state, pipeline_id).await?;

        self.run_pipeline_impl(&state, pipeline.clone()).await?;

        Ok(())
    }

    pub async fn run_service_actions<'a>(
        &self,
        state: &State<'a>,
        actions: HashMap<String, config::actions::ServiceAction>,
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
                config::actions::ServiceAction::Deploy => {
                    service.get_restart_job(/* build */ true).ok_or_else(|| {
                        anyhow!("Cannot construct deploy config for service {}", service_id)
                    })?
                }
                config::actions::ServiceAction::Start { build } => {
                    service.get_start_job(build).ok_or_else(|| {
                        anyhow!("Cannot construct start config for service {}", service_id)
                    })?
                }
                config::actions::ServiceAction::Stop => {
                    service.get_stop_job().ok_or_else(|| {
                        anyhow!("Cannot construct stop config for service {}", service_id)
                    })?
                }
                config::actions::ServiceAction::Restart { build } => {
                    service.get_restart_job(build).ok_or_else(|| {
                        anyhow!("Cannot construct restart config for service {}", service_id)
                    })?
                }
                config::actions::ServiceAction::Logs { follow, tail } => {
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
            networks: self.services.networks.values().cloned().collect(),
            volumes: self.services.volumes.values().cloned().collect(),
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

        let current_project = worker_lib::executor::CurrentProject(self.id.clone());
        let mut state = state.clone();
        state.set(&current_project);

        info!("Running pipeline {}", pipeline_id);

        let pinfo: &config::projects::ProjectInfo = state.get()?;
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
        event: &config::actions::Event,
    ) -> Result<(), anyhow::Error> {
        let EventActions {
            run_pipelines,
            services,
            params,
        } = self.actions.get_matched_actions(event).await?;
	let action_params = ActionParams(params);
	let mut state = state.clone();
	state.set(&action_params);

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

pub struct ProjectParams(pub dynconf::Value);
pub struct ActionParams(pub dynconf::Value);

pub mod raw {
    use crate::config;

    use dynconf::*;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    use anyhow::{anyhow, Result};

    #[derive(Deserialize, Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct Project {
        actions: Option<util::Dyn<config::actions::raw::Actions>>,
        pipelines: Option<util::Dyn<config::pipelines::raw::Pipelines>>,
        docker: Option<util::Dyn<config::services::raw::Services>>,
        bind: Option<util::OneOrMany<util::Dyn<config::bind::raw::Bind>>>,
        caddy: Option<util::OneOrMany<util::Dyn<config::caddy::raw::Caddy>>>,
        params: Option<util::Dyn<HashMap<String, util::DynAny>>>,
    }

    #[async_trait::async_trait]
    impl util::DynValue for Project {
        type Target = super::Project;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            let dynobj = config::utils::get_dyn_object(state)?;
            let env = dynobj.env;
            let project_id = dynobj
                .project
                .ok_or_else(|| anyhow!("No project binding"))?
                .id;

            let params_raw = self.params.load(state).await?.unwrap_or_default();
            let params = params_raw
                .get("__default__")
                .cloned()
                .unwrap_or_default()
                .merge(params_raw.get(&env).cloned().unwrap_or_default())?;

            state.mutate_global(config::utils::wrap_dyn_f(|mut dynobj| {
                dynobj.params = dynobj.params.merge(params.clone())?;
                Ok(dynobj)
            }))?;

            let bind: Vec<config::bind::Bind> = self
                .bind
                .load(state)
                .await?
                .unwrap_or_default()
                .into_iter()
                .filter_map(|v| v)
                .collect();
            let caddy: Vec<config::caddy::Caddy> = self
                .caddy
                .load(state)
                .await?
                .unwrap_or_default()
                .into_iter()
                .filter_map(|v| v)
                .collect();

            let services = self.docker.load(state).await?.unwrap_or_default();

            state.mutate_global(config::utils::wrap_dyn_f(|mut dynobj| {
                dynobj.services = Some((&services).into());
                Ok(dynobj)
            }))?;

            let actions = self.actions.load(state).await?.unwrap_or_default();
            let pipelines = self.pipelines.load(state).await?.unwrap_or_default();

            Ok(super::Project {
                id: project_id.clone(),
                bind,
                caddy,
                params,
                actions,
                services,
                pipelines,
            })
        }
    }
}
