use std::{collections::HashMap, path::PathBuf, sync::Arc};

use super::LoadConfigError;

#[derive(Debug)]
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
    pub async fn load<'a>(
        context: &super::State<'a>,
    ) -> Result<ServiceConfig, LoadConfigError> {
        raw::load(context).await
    }

    pub fn check_allowed<S: AsRef<str>, PS: AsRef<str>>(
        &self,
        token: Option<S>,
        action: super::ActionType,
    ) -> bool {
        self.tokens.check_allowed(token, action)
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
            context: &config::State,
        ) -> Result<Self::Output, config::LoadConfigError> {
            let service_config: PathBuf = context.get_named("service_config").cloned()?;
            let data_dir =
                utils::try_expand_home(self.data_dir.unwrap_or(DEFAULT_DATA_DIR.to_string()));

            let secrets = if let Some(secrets) = self.secrets {
                let secrets_path = utils::eval_rel_path(&context, secrets, service_config)?;
                config::Secrets::load(secrets_path).await?
            } else {
                config::Secrets::default()
            };

            let mut context = context.clone();
            context.set(&secrets);

            let tokens = if let Some(tokens) = self.tokens {
                tokens.load_raw(&context)?
            } else {
                config::Tokens::default()
            };

            Ok(super::ServiceConfig {
                data_path: data_dir.join(DEFAULT_DATA_PATH),
                repos_path: data_dir.join(DEFAULT_REPOS_PATH),
                internal_path: data_dir.join(DEFAULT_INTERNAL_PATH),
                worker_url: self.worker_url,
                secrets,
                tokens,
                data_dir,
            })
        }
    }

    pub async fn load<'a>(
        context: &config::State<'a>,
    ) -> Result<super::ServiceConfig, super::LoadConfigError> {
        let service_config: PathBuf = context.get_named("service_config").cloned()?;
        config::load::<ServiceConfig>(service_config.clone(), context)
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
