use std::{collections::HashMap, path::PathBuf};

use super::LoadConfigError;

#[derive(Debug)]
pub struct Services {
    services: HashMap<String, Service>,
    pub networks: HashMap<String, Network>,
    pub volumes: HashMap<String, Volume>,
}

#[derive(Debug)]
pub struct Network {
    pub global: bool,
}

#[derive(Debug)]
pub struct Volume {
    pub global: bool,
}

#[derive(Debug)]
pub struct Service {
    container: String,
    build: Option<Build>,
    image: String,
    volumes: HashMap<String, String>,
    networks: Vec<String>,
}

#[derive(Debug)]
struct Build {
    path: PathBuf,
    dockerfile: Option<String>,
    context: Option<String>,
}

impl Services {
    pub async fn load<'a>(context: &super::LoadContext<'a>) -> Result<Services, LoadConfigError> {
        raw::load(context).await
    }

    pub fn get(&self, service: &str) -> Option<&Service> {
        self.services.get(service)
    }
}

impl Service {
    pub fn get_build_config(&self) -> Option<common::BuildImageConfig> {
        let source = self
            .build
            .as_ref()
            .map(|build| common::BuildImageConfigSource {
                dockerfile: build.dockerfile.clone(),
                path: common::BuildImageConfigSourcePath::Directory(
                    build.path.to_string_lossy().to_string(),
                ),
                context: build.context.clone(),
            });

        Some(common::BuildImageConfig {
            image: self.image.clone(),
            tag: None, // FIXME: Specify somewhere
            source,
        })
    }

    pub fn get_run_config(&self) -> Option<common::RunContainerConfig> {
        let volumes = self.volumes.clone();
        let networks = self.networks.clone();

        Some(common::RunContainerConfig {
            name: self.image.clone(),
            image: self.container.clone(),
            volumes,
            networks,
        })
    }

    pub fn get_stop_config(&self) -> Option<common::StopContainerConfig> {
        Some(common::StopContainerConfig {
            name: self.container.clone(),
        })
    }
}

mod raw {
    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};

    use crate::lib::config;

    const SERVICES_CONFIG: &str = "services.yaml";

    #[derive(Serialize, Deserialize)]
    struct Services {
        services: HashMap<String, Service>,
        networks: HashMap<String, Network>,
        volumes: HashMap<String, Volume>,
    }

    #[derive(Serialize, Deserialize)]
    struct Service {
        #[serde(default = "default_global")]
        global: bool,
        build: Option<Build>,
        image: Option<String>,
        volumes: Option<HashMap<String, String>>,
        networks: Option<Vec<String>>,
    }

    #[derive(Serialize, Deserialize)]
    struct Build {
        repo: String,
        dockerfile: Option<String>,
        context: Option<String>,
    }

    #[derive(Serialize, Deserialize)]
    struct Network {
        #[serde(default = "default_global")]
        global: bool,
    }

    #[derive(Serialize, Deserialize)]
    struct Volume {
        #[serde(default = "default_global")]
        global: bool,
    }

    fn default_global() -> bool {
        false
    }

    impl config::LoadRawSync for Services {
        type Output = super::Services;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            let networks = self.networks.load_raw(context)?;
            let volumes = self.volumes.load_raw(context)?;

            let services: Result<HashMap<_, _>, super::LoadConfigError> = self
                .services
                .into_iter()
                .map(|(id, service)| {
                    let mut context = context.clone();
                    context.set_service_id(&id);
                    context.set_networks(&networks);
                    context.set_volumes(&volumes);
                    let service = service.load_raw(&context)?;
                    Ok((id, service))
                })
                .collect();
            let services = services?;

            Ok(super::Services {
                services,
                networks,
                volumes,
            })
        }
    }

    impl config::LoadRawSync for Network {
        type Output = super::Network;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            Ok(super::Network {
                global: self.global,
            })
        }
    }

    impl config::LoadRawSync for Volume {
        type Output = super::Volume;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            Ok(super::Volume {
                global: self.global,
            })
        }
    }

    impl config::LoadRawSync for Service {
        type Output = super::Service;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            let build = if let Some(build) = self.build {
                Some(build.load_raw(context)?)
            } else {
                None
            };

            let networks =
                config::utils::get_networks_names(context, self.networks.unwrap_or_default())?;
            let volumes =
                config::utils::get_volumes_names(context, self.volumes.unwrap_or_default())?;

            Ok(super::Service {
                image: get_image_name(context, self.image, self.global)?,
                container: get_container_name(context, self.global)?,
                networks,
                volumes,
                build,
            })
        }
    }

    fn get_image_name(
        context: &config::LoadContext,
        image: Option<String>,
        global: bool,
    ) -> Result<String, config::LoadConfigError> {
        if let Some(image) = image {
            // Will pull specified image
            Ok(String::from(image))
        } else if global {
            // Image name is service name
            Ok(String::from(context.service_id()?))
        } else {
            // Image name is scoped under project
            Ok(format!(
                "{}_{}",
                context.project_id()?,
                context.service_id()?
            ))
        }
    }

    fn get_container_name(
        context: &config::LoadContext,
        global: bool,
    ) -> Result<String, super::LoadConfigError> {
        if global {
            // Container name is service name
            Ok(String::from(context.service_id()?))
        } else {
            // Container name is scoped under project
            Ok(format!(
                "{}_{}",
                context.project_id()?,
                context.service_id()?
            ))
        }
    }

    impl config::LoadRawSync for Build {
        type Output = super::Build;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            Ok(super::Build {
                path: context.config()?.repos_path.join(self.repo.clone()),
                dockerfile: self.dockerfile,
                context: self.context,
            })
        }
    }

    pub async fn load<'a>(
        context: &config::LoadContext<'a>,
    ) -> Result<super::Services, super::LoadConfigError> {
        config::load_sync::<Services>(context.project_root()?.join(SERVICES_CONFIG), context).await
    }
}
