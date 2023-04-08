use std::{collections::HashMap, path::PathBuf};

use super::LoadConfigError;

#[derive(Debug)]
pub struct ServiceConfig {
    pub data_dir: PathBuf,
    pub repos_path: PathBuf,
    pub data_path: PathBuf,
    pub internal_path: PathBuf,
    pub worker_url: Option<String>,
    pub secrets: HashMap<String, String>,
}

impl ServiceConfig {
    pub async fn load<'a>(
        context: &super::LoadContext<'a>,
    ) -> Result<ServiceConfig, LoadConfigError> {
        raw::load(context).await
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::lib::{config, utils};

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
    }

    #[async_trait::async_trait]
    impl config::LoadRaw for ServiceConfig {
        type Output = super::ServiceConfig;

        async fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            let data_dir =
                utils::try_expand_home(self.data_dir.unwrap_or(DEFAULT_DATA_DIR.to_string()));
            let secrets = if let Some(secrets) = self.secrets {
                let secrets_path =
                    utils::abs_or_rel_to_dir(secrets, context.configs_root()?.clone());
                Some(load_secrets(secrets_path).await?)
            } else {
                None
            };

            Ok(super::ServiceConfig {
                data_path: data_dir.join(DEFAULT_DATA_PATH),
                repos_path: data_dir.join(DEFAULT_REPOS_PATH),
                internal_path: data_dir.join(DEFAULT_INTERNAL_PATH),
                worker_url: self.worker_url,
                secrets: secrets.unwrap_or_default(),
                data_dir,
            })
        }
    }

    async fn load_secrets(
        path: PathBuf,
    ) -> Result<HashMap<String, String>, config::LoadConfigError> {
        let content = tokio::fs::read_to_string(path).await?;
        Ok(serde_yaml::from_str(&content)?)
    }

    pub async fn load<'a>(
        context: &config::LoadContext<'a>,
    ) -> Result<super::ServiceConfig, super::LoadConfigError> {
        let path = context.configs_root()?.join(SERVICE_CONFIG);
        config::load::<ServiceConfig>(path.clone(), context)
            .await
            .map_err(|err| {
                anyhow::anyhow!("Failed to load service_config from {:?}: {}", path, err).into()
            })
    }
}
