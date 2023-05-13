use std::collections::HashMap;

use std::path::PathBuf;

use common::state::State;
use log::*;

pub const INTERNAL_DATA_DIR: &str = "internal";
pub const BIND9_DATA_DIR: &str = "bind9";
pub const CADDY_DATA_DIR: &str = "caddy";
pub const INTERNAL_PROJECT_DATA_DIR: &str = "internal_project";

#[derive(Clone)]
pub struct StaticProjects {
    pub projects_config: PathBuf,
}

#[async_trait::async_trait]
impl super::ProjectsManager for StaticProjects {
    async fn get_project_info<'a>(
        &mut self,
        state: &State<'a>,
        project_id: &str,
    ) -> Result<super::ProjectInfo, anyhow::Error> {
        if let Some(project) = self.load_projects_info(state).await?.remove(project_id) {
            Ok(project)
        } else {
            Err(anyhow::anyhow!("No such project {}", project_id))
        }
    }

    async fn list_projects<'a>(
        &mut self,
        state: &State<'a>,
    ) -> Result<Vec<super::ProjectInfo>, anyhow::Error> {
        Ok(self
            .load_projects_info(state)
            .await?
            .into_iter()
            .map(|(k, v)| v)
            .collect())
    }
}

impl From<&StaticProjects> for common::vars::Value {
    fn from(val: &StaticProjects) -> Self {
        let mut vars = common::vars::Value::default();

        vars.assign(
            "config",
            val.projects_config.to_string_lossy().to_string().into(),
        )
        .ok();

        vars.assign(
            "config_dir",
            val.projects_config
                .parent()
                .unwrap()
                .to_string_lossy()
                .to_string()
                .into(),
        )
        .ok();

        vars
    }
}

impl StaticProjects {
    pub async fn new(projects_config: PathBuf) -> Result<StaticProjects, anyhow::Error> {
        Ok(Self { projects_config })
    }

    pub async fn load_projects_info<'a>(
        &self,
        state: &State<'a>,
    ) -> Result<HashMap<String, super::ProjectInfo>, anyhow::Error> {
        let mut state = state.clone();
        state.set(self);
        let res = raw::load(&state).await?;
        debug!("Loaded static projects: {:#?}", res);
        Ok(res)
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use crate::config;

    use common::state::State;
    use config::LoadRawSync;
    use serde::{Deserialize, Serialize};

    use anyhow::anyhow;

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Projects {
        projects: HashMap<String, Project>,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Project {
        enabled: Option<bool>,
        path: config::OneOrMany<config::AbsPath>,
        #[serde(default)]
        repos: HashMap<String, config::repos_raw::Repo>,
        secrets: Option<config::OneOrMany<config::File<config::secrets::raw::Secrets>>>,
        tokens: Option<config::permissions_raw::Tokens>,
    }

    #[async_trait::async_trait]
    impl config::LoadRaw for Projects {
        type Output = super::HashMap<String, config::ProjectInfo>;

        async fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            self.projects.load_raw(state).await
        }
    }

    #[async_trait::async_trait]
    impl config::LoadRaw for Project {
        type Output = config::ProjectInfo;

        async fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            let service_config: &config::ServiceConfig = state.get()?;
            let project_id: String = state.get_named("_id").cloned()?;

            let mut res = config::ProjectInfo {
                data_path: service_config.data_path.join(&project_id),
                id: project_id,
                enabled: true,
                ..Default::default()
            };

            res.repos = {
                let mut state = state.clone();
                state.set(&res);
                config::Repos {
                    repos: self
                        .repos
                        .load_raw(&state)
                        .await
                        .map_err(|err| anyhow!("Failed to load repos config: {}", err))?,
                }
            };

            res.path = {
                let mut state = state.clone();
                state.set(&res);
                self.path.load_raw(&state)?
            };

            res.secrets = {
                let mut state = state.clone();
                state.set(&res);
                let secrets: Vec<config::Secrets> = self
                    .secrets
                    .load_raw(&state)
                    .await
                    .map_err(|err| anyhow!("Failed to load secrets: {}", err))?
                    .unwrap_or_default();

                secrets
                    .into_iter()
                    .try_fold(config::Secrets::default(), config::Secrets::merge)?
            };

            res.tokens = {
                let mut state = state.clone();
                state.set(&res);
                self.tokens.load_raw(&state)?.unwrap_or_default()
            };

            Ok(res)
        }
    }

    pub async fn load<'a>(
        state: &State<'a>,
    ) -> Result<HashMap<String, config::ProjectInfo>, anyhow::Error> {
        let static_projects: &config::StaticProjects = state.get()?;
        let path: PathBuf = static_projects.projects_config.clone();
        config::load::<Projects>(path.clone(), state)
            .await
            .map_err(|err| {
                anyhow::anyhow!("Failed to load projects config from {:?}: {}", path, err)
            })
    }
}
