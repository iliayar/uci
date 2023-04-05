use std::path::PathBuf;

use super::LoadConfigError;

#[derive(Debug)]
pub struct ServiceConfig {
    pub data_dir: PathBuf,
    pub repos_path: PathBuf,
    pub data_path: PathBuf,
    pub worker_url: Option<String>,
}

impl ServiceConfig {
    pub async fn load<'a>(
        context: &super::LoadContext<'a>,
    ) -> Result<ServiceConfig, LoadConfigError> {
        raw::load(context).await
    }
}

mod raw {
    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use crate::lib::{config, utils};

    const SERVICE_CONFIG: &str = "conf.yaml";

    const DEFAULT_REPOS_PATH: &str = "repos";
    const DEFAULT_DATA_PATH: &str = "data";
    const DEFAULT_DATA_DIR: &str = "~/.microci";

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ServiceConfig {
        data_dir: Option<String>,
        worker_url: Option<String>,
    }

    impl config::LoadRawSync for ServiceConfig {
        type Output = super::ServiceConfig;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            let data_dir =
                utils::try_expand_home(self.data_dir.unwrap_or(DEFAULT_DATA_DIR.to_string()));
            Ok(super::ServiceConfig {
                data_path: data_dir.join(DEFAULT_DATA_PATH),
                repos_path: data_dir.join(DEFAULT_REPOS_PATH),
                worker_url: self.worker_url,
                data_dir,
            })
        }
    }

    pub async fn load<'a>(
        context: &config::LoadContext<'a>,
    ) -> Result<super::ServiceConfig, super::LoadConfigError> {
        config::load_sync::<ServiceConfig>(context.configs_root()?.join(SERVICE_CONFIG), context)
            .await
    }
}
