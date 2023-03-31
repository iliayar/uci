use std::{collections::HashMap, path::PathBuf};

use super::LoadConfigError;

const SERVICES_CONFIG: &str = "services.yaml";

#[derive(Debug)]
pub struct Services {
    services: HashMap<String, Service>,
}

#[derive(Debug)]
struct Service {
    global: bool,
    build: Option<Build>,
    image: Option<String>,
}

#[derive(Debug)]
struct Build {
    repo: String,
    dockerfile: Option<String>,
}

impl Services {
    pub async fn load(project_root: PathBuf) -> Result<Services, LoadConfigError> {
        raw::parse(project_root.join(SERVICES_CONFIG)).await
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::lib::{config, utils};

    #[derive(Serialize, Deserialize)]
    struct Services {
        services: HashMap<String, Service>,
    }

    #[derive(Serialize, Deserialize)]
    struct Service {
        #[serde(default = "default_global")]
        global: bool,
        build: Option<Build>,
	image: Option<String>,
    }

    #[derive(Serialize, Deserialize)]
    struct Build {
        repo: String,
        dockerfile: Option<String>,
    }

    fn default_global() -> bool {
        false
    }

    impl TryFrom<Services> for super::Services {
        type Error = super::LoadConfigError;

        fn try_from(value: Services) -> Result<Self, Self::Error> {
            let mut services = HashMap::new();
            for (k, v) in value.services.into_iter() {
                services.insert(k, v.try_into()?);
            }

            Ok(super::Services { services })
        }
    }

    impl TryFrom<Service> for super::Service {
        type Error = super::LoadConfigError;

        fn try_from(value: Service) -> Result<Self, Self::Error> {
            let build = if let Some(build) = value.build {
                Some(build.try_into()?)
            } else {
                None
            };

            Ok(super::Service {
                global: value.global,
		image: value.image,
                build,
            })
        }
    }

    impl TryFrom<Build> for super::Build {
        type Error = super::LoadConfigError;

        fn try_from(value: Build) -> Result<Self, Self::Error> {
            Ok(super::Build {
                repo: value.repo,
                dockerfile: value.dockerfile,
            })
        }
    }

    pub async fn parse(path: PathBuf) -> Result<super::Services, super::LoadConfigError> {
        config::utils::load_file::<Services, _>(path).await
    }
}
