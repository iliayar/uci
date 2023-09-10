use std::collections::HashMap;

use anyhow::Result;
use common::state::State;
use dynconf::DynValue;
use log::*;

use crate::config;

pub const INTERNAL_DATA_DIR: &str = "internal";

#[derive(Clone)]
pub struct StaticProjects {
    pub projects_lazy: dynconf::util::LoadedLazy<raw::Projects>,
}

#[async_trait::async_trait]
impl config::projects::ProjectsManager for StaticProjects {
    async fn get_project_info<'a>(
        &mut self,
        state: &State<'a>,
        project_id: &str,
    ) -> Result<config::projects::ProjectInfo> {
        if let Some(project) = self.load_projects_info(state).await?.remove(project_id) {
            Ok(project)
        } else {
            Err(anyhow::anyhow!("No such project {}", project_id))
        }
    }

    async fn list_projects<'a>(
        &mut self,
        state: &State<'a>,
    ) -> Result<Vec<config::projects::ProjectInfo>> {
        Ok(self
            .load_projects_info(state)
            .await?
            .into_iter()
            .map(|(k, v)| v)
            .collect())
    }
}

impl StaticProjects {
    pub async fn new(
        projects_lazy: dynconf::util::LoadedLazy<raw::Projects>,
    ) -> Result<StaticProjects, anyhow::Error> {
        Ok(Self { projects_lazy })
    }

    pub async fn load_projects_info<'a>(
        &self,
        state: &State<'a>,
    ) -> Result<HashMap<String, config::projects::ProjectInfo>> {
        let mut dyn_state = config::utils::make_dyn_state(state)?;
        let res = self.projects_lazy.clone().load(&mut dyn_state).await?;
        debug!("Loaded static projects: {:#?}", res);
        Ok(res)
    }
}

pub mod raw {
    use crate::config;

    use dynconf::*;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    use anyhow::{anyhow, Result};

    #[derive(Deserialize, Serialize, Clone)]
    #[serde(transparent)]
    pub struct Projects {
        projects: HashMap<String, util::Dyn<Project>>,
    }

    #[derive(Deserialize, Serialize, Clone)]
    #[serde(deny_unknown_fields)]
    struct Project {
        enabled: Option<util::Dyn<bool>>,
        config: util::OneOrMany<util::Dyn<util::Lazy<config::project::raw::Project>>>,
        repos: Option<util::Dyn<config::repo::raw::Repos>>,
        secrets: Option<util::OneOrMany<util::Dyn<config::secrets::raw::Secrets>>>,
        tokens: Option<util::Dyn<config::permissions::raw::Tokens>>,
    }

    #[async_trait::async_trait]
    impl util::DynValue for Project {
        type Target = config::projects::ProjectInfo;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            let dynobj = config::utils::get_dyn_object(state)?;
            let project_id = dynobj._id.ok_or_else(|| anyhow!("No _id binding"))?;
            let data_path = dynobj
                .config
                .ok_or_else(|| anyhow!("No config binding"))?
                .data_path
                .join(&project_id);

            state.mutate_global(config::utils::wrap_dyn_f(|mut dynobj| {
                dynobj.project = Some(config::projects::DynProjectInfo {
                    id: project_id.clone(),
                    data_path: data_path.clone(),
                    secrets: None,
                    repos: None,
                });
                Ok(dynobj)
            }))?;

            let repos = self.repos.load(state).await?.unwrap_or_default();

            state.mutate_global(config::utils::wrap_dyn_f(|mut dynobj| {
                dynobj.project.as_mut().unwrap().repos = Some((&repos).into());
                Ok(dynobj)
            }))?;

            let projects = self.config.load(state).await?;

            let secrets: config::secrets::Secrets = self
                .secrets
                .load(state)
                .await?
                .unwrap_or_default()
                .into_iter()
                .try_fold(
                    config::secrets::Secrets::default(),
                    config::secrets::Secrets::merge,
                )?;

            state.mutate_global(config::utils::wrap_dyn_f(|mut dynobj| {
                dynobj.project.as_mut().unwrap().secrets = Some((&secrets).into());
                Ok(dynobj)
            }))?;

            let tokens = self.tokens.load(state).await?.unwrap_or_default();

            Ok(config::projects::ProjectInfo {
                id: project_id,
                enabled: self.enabled.load(state).await?.unwrap_or(true),
                projects,
                repos,
                tokens,
                secrets,
                data_path,
            })
        }
    }

    #[async_trait::async_trait]
    impl util::DynValue for Projects {
        type Target = HashMap<String, config::projects::ProjectInfo>;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            self.projects.load(state).await
        }
    }
}
