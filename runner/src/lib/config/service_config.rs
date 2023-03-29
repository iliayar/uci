use std::path::PathBuf;

use super::LoadConfigError;

const SERVICE_CONFIG: &str = "repos.yaml";

#[derive(Debug)]
pub struct ServiceConfig {
    pub repos_path: PathBuf,
    pub worker_url: Option<String>,
}

impl ServiceConfig {
    pub async fn load(configs_root: PathBuf) -> Result<ServiceConfig, LoadConfigError> {
        raw::parse(configs_root.join(SERVICE_CONFIG)).await
    }
}

mod raw {
    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use crate::lib::{config, utils};

    #[derive(Serialize, Deserialize)]
    struct ServiceConfig {
        repos_path: String,
        worker_url: Option<String>,
    }

    impl TryFrom<ServiceConfig> for super::ServiceConfig {
        type Error = super::LoadConfigError;

        fn try_from(value: ServiceConfig) -> Result<Self, Self::Error> {
            Ok(super::ServiceConfig {
                repos_path: utils::try_expand_home(value.repos_path),
                worker_url: value.worker_url,
            })
        }
    }

    pub async fn parse(path: PathBuf) -> Result<super::ServiceConfig, super::LoadConfigError> {
        config::utils::load_file::<ServiceConfig, _>(path).await
    }
}
