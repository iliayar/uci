use std::path::PathBuf;

use super::LoadConfigError;

const SERVICE_CONFIG: &str = "conf.yaml";

const DEFAULT_REPOS_PATH: &str = "repos";
const DEFAULT_DATA_PATH: &str = "data";
const DEFAULT_DATA_DIR: &str = "~/.microci";

#[derive(Debug)]
pub struct ServiceConfig {
    pub data_dir: PathBuf,
    pub repos_path: PathBuf,
    pub data_path: PathBuf,
    pub worker_url: Option<String>,
}

impl ServiceConfig {
    pub async fn load<'a>(
        configs_root: PathBuf,
        context: &mut super::LoadContext<'a>,
    ) -> Result<ServiceConfig, LoadConfigError> {
        super::load::<raw::ServiceConfig>(configs_root.join(SERVICE_CONFIG), context).await
    }
}

mod raw {
    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use crate::lib::{config, utils};

    #[derive(Serialize, Deserialize)]
    pub struct ServiceConfig {
        data_dir: Option<String>,
        worker_url: Option<String>,
    }

    impl config::LoadRaw for ServiceConfig {
        type Output = super::ServiceConfig;

        fn load_raw(
            self,
            context: &mut config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            let data_dir = utils::try_expand_home(
                self.data_dir.unwrap_or(super::DEFAULT_DATA_DIR.to_string()),
            );
            Ok(super::ServiceConfig {
                data_path: data_dir.join(super::DEFAULT_DATA_PATH),
                repos_path: data_dir.join(super::DEFAULT_REPOS_PATH),
                worker_url: self.worker_url,
                data_dir,
            })
        }
    }

    pub async fn load<'a>(
        path: PathBuf,
        context: &mut config::LoadContext<'a>,
    ) -> Result<super::ServiceConfig, super::LoadConfigError> {
        config::load::<ServiceConfig>(path, context).await
    }
}
