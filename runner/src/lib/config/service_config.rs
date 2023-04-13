use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::Error;

#[derive(Debug, Default)]
pub struct ServiceConfig {
    pub data_dir: PathBuf,
    pub repos_path: PathBuf,
    pub data_path: PathBuf,
    pub internal_path: PathBuf,
    pub worker_url: Option<String>,
    pub secrets: super::Secrets,
    pub tokens: super::Tokens,
}

pub enum ActionEvent {
    ConfigReloaded,
    ProjectReloaded {
        project_id: String,
    },
    DirectCall {
        project_id: String,
        trigger_id: String,
    },
    UpdateRepos {
        project_id: String,
        repos: Vec<String>,
    },
}

impl ServiceConfig {
    pub async fn load<'a>(state: &super::State<'a>) -> Result<ServiceConfig, anyhow::Error> {
        raw::load(state).await
    }

    pub fn check_allowed<S: AsRef<str>>(
        &self,
        token: Option<S>,
        action: super::ActionType,
    ) -> bool {
        self.tokens.check_allowed(token, action)
    }
}

impl Into<common::vars::Vars> for &ServiceConfig {
    fn into(self) -> common::vars::Vars {
        let mut vars = common::vars::Vars::default();
        vars.assign("internal.path", (&self.internal_path).into())
            .ok();
        vars.assign("secrets", (&self.secrets).into()).ok();
        vars
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::lib::config::load::LoadRawSync;
    use crate::lib::{config, utils};

    use log::*;

    const SERVICE_CONFIG: &str = "conf.yaml";

    const DEFAULT_REPOS_PATH: &str = "repos";
    const DEFAULT_DATA_PATH: &str = "data";
    const DEFAULT_INTERNAL_PATH: &str = "internal";
    const DEFAULT_DATA_DIR: &str = "~/.microci";

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ServiceConfig {
        data_dir: Option<String>,
        worker_url: Option<String>,
        secrets: Option<String>,
        tokens: Option<config::permissions_raw::Tokens>,
    }

    #[async_trait::async_trait]
    impl config::LoadRaw for ServiceConfig {
        type Output = super::ServiceConfig;

        async fn load_raw(
            self,
            state: &config::State,
        ) -> Result<Self::Output, anyhow::Error> {
            let service_config: PathBuf = state.get_named("service_config").cloned()?;
            let data_dir =
                utils::try_expand_home(self.data_dir.unwrap_or(DEFAULT_DATA_DIR.to_string()));

            let mut res = super::ServiceConfig {
                data_dir: data_dir.clone(),
                data_path: data_dir.join(DEFAULT_DATA_PATH),
                repos_path: data_dir.join(DEFAULT_REPOS_PATH),
                internal_path: data_dir.join(DEFAULT_INTERNAL_PATH),
                worker_url: self.worker_url,
                ..Default::default()
            };

            res.secrets = {
                let mut state = state.clone();
                state.set(&res);

                if let Some(secrets) = self.secrets {
                    let secrets_path = utils::eval_rel_path(&state, secrets, service_config)?;
                    config::Secrets::load(secrets_path).await?
                } else {
                    config::Secrets::default()
                }
            };

            res.tokens = {
                let mut state = state.clone();
                state.set(&res);

                if let Some(tokens) = self.tokens {
                    tokens.load_raw(&state)?
                } else {
                    config::Tokens::default()
                }
            };

            Ok(res)
        }
    }

    pub async fn load<'a>(
        state: &config::State<'a>,
    ) -> Result<super::ServiceConfig, anyhow::Error> {
        let service_config: PathBuf = state.get_named("service_config").cloned()?;
        config::load::<ServiceConfig>(service_config.clone(), state)
            .await
            .map_err(|err| {
                anyhow::anyhow!(
                    "Failed to load service_config from {:?}: {}",
                    service_config,
                    err
                )
                .into()
            })
    }
}
