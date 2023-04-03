use std::{collections::HashMap, path::PathBuf};

use super::LoadConfigError;

const SERVICES_CONFIG: &str = "services.yaml";

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
    id: String,
    global: bool,
    build: Option<Build>,
    image: Option<String>,
    volumes: HashMap<String, String>,
    networks: Vec<String>,
}

#[derive(Debug)]
struct Build {
    repo: String,
    dockerfile: Option<String>,
    context: Option<String>,
}

impl Services {
    pub async fn load(project_root: PathBuf) -> Result<Services, LoadConfigError> {
        raw::parse(project_root.join(SERVICES_CONFIG)).await
    }

    pub fn get(&self, service: &str) -> Option<&Service> {
        self.services.get(service)
    }

    pub fn get_network_name(&self, project: &super::Project, name: &str) -> String {
        if let Some(config) = self.networks.get(name) {
            get_resource_name(project, name, config.global)
        } else {
            name.to_string()
        }
    }

    pub fn get_volume_name(&self, project: &super::Project, name: &str) -> String {
        if let Some(config) = self.volumes.get(name) {
            get_resource_name(project, name, config.global)
        } else {
            name.to_string()
        }
    }
}

pub fn get_resource_name(project: &super::Project, name: &str, global: bool) -> String {
    if global {
        name.to_string()
    } else {
        format!("{}_{}", project.id, name)
    }
}

impl Service {
    pub fn get_build_config(
        &self,
        project: &super::Project,
        config: &super::ServiceConfig,
    ) -> Option<common::BuildImageConfig> {
        let image = self.get_image_name(project);
        let source = self
            .build
            .as_ref()
            .map(|build| common::BuildImageConfigSource {
                dockerfile: build.dockerfile.clone(),
                path: common::BuildImageConfigSourcePath::Directory(
                    config
                        .repos_path
                        .join(build.repo.clone())
                        .to_string_lossy()
                        .to_string(),
                ),
                context: build.context.clone(),
            });

        Some(common::BuildImageConfig {
            image,
            tag: None, // FIXME: Specify somewhere
            source,
        })
    }

    pub fn get_run_config(
        &self,
        project: &super::Project,
        config: &super::ServiceConfig,
    ) -> Option<common::RunContainerConfig> {
        let image = self.get_image_name(project);
        let name = self.get_container_name(project);
        let volumes = super::utils::prepare_links(&project.id, config, &self.volumes);
        let volumes = volumes
            .into_iter()
            .map(|(k, v)| (project.services.get_volume_name(project, &v), k))
            .collect();
        let networks = self
            .networks
            .iter()
            .map(|name| project.services.get_network_name(project, name))
            .collect();

        Some(common::RunContainerConfig {
            name,
            image,
            volumes,
            networks,
        })
    }

    pub fn get_stop_config(&self, project: &super::Project) -> Option<common::StopContainerConfig> {
        let name = self.get_container_name(project);

        Some(common::StopContainerConfig { name })
    }

    fn get_image_name(&self, project: &super::Project) -> String {
        if let Some(image) = &self.image {
            // Will pull specified image
            String::from(image)
        } else if self.global {
            // Image name is service name
            String::from(&self.id)
        } else {
            // Image name is scoped under project
            format!("{}_{}", project.id, self.id)
        }
    }

    fn get_container_name(&self, project: &super::Project) -> String {
        if self.global {
            // Container name is service name
            String::from(&self.id)
        } else {
            // Container name is scoped under project
            format!("{}_{}", project.id, self.id)
        }
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::lib::{config, utils};

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
        global: Option<bool>,
    }

    #[derive(Serialize, Deserialize)]
    struct Volume {
        global: Option<bool>,
    }

    fn default_global() -> bool {
        false
    }

    impl TryFrom<Services> for super::Services {
        type Error = super::LoadConfigError;

        fn try_from(value: Services) -> Result<Self, Self::Error> {
            fn convert_service(
                (k, v): (String, Service),
            ) -> Result<(String, super::Service), super::LoadConfigError> {
                let build = if let Some(build) = v.build {
                    Some(build.try_into()?)
                } else {
                    None
                };

                Ok((
                    k.clone(),
                    super::Service {
                        id: k.clone(),
                        global: v.global,
                        image: v.image,
                        networks: v.networks.unwrap_or_default(),
                        volumes: v.volumes.unwrap_or_default(),
                        build,
                    },
                ))
            }

            let services: Result<HashMap<_, _>, super::LoadConfigError> =
                value.services.into_iter().map(convert_service).collect();
            let services = services?;

            let networks = value
                .networks
                .into_iter()
                .map(|(k, Network { global })| {
                    (
                        k,
                        super::Network {
                            global: global.unwrap_or(false),
                        },
                    )
                })
                .collect();

            let volumes = value
                .volumes
                .into_iter()
                .map(|(k, Volume { global })| {
                    (
                        k,
                        super::Volume {
                            global: global.unwrap_or(false),
                        },
                    )
                })
                .collect();

            Ok(super::Services {
                services,
                networks,
                volumes,
            })
        }
    }

    impl TryFrom<Build> for super::Build {
        type Error = super::LoadConfigError;

        fn try_from(value: Build) -> Result<Self, Self::Error> {
            Ok(super::Build {
                repo: value.repo,
                dockerfile: value.dockerfile,
                context: value.context,
            })
        }
    }

    pub async fn parse(path: PathBuf) -> Result<super::Services, super::LoadConfigError> {
        config::utils::load_file::<Services, _>(path).await
    }
}
