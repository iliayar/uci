use std::{collections::HashMap, path::PathBuf};

use super::LoadConfigError;

use anyhow::anyhow;
use log::*;

#[derive(Debug, Default)]
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
    ports: Vec<common::PortMapping>,
    command: Option<Vec<String>>,
    restart: String,
    env: HashMap<String, String>,
}

#[derive(Debug)]
struct Build {
    path: PathBuf,
    dockerfile: Option<String>,
}

pub const SERVICES_CONFIG: &str = "services.yaml";

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
            ports: self.ports.clone(),
            command: self.command.clone(),
            restart_policy: self.restart.clone(),
            env: self.env.clone(),
            volumes,
            networks,
        })
    }

    pub fn get_stop_config(&self) -> Option<common::StopContainerConfig> {
        Some(common::StopContainerConfig {
            name: self.container.clone(),
        })
    }

    pub fn get_deploy_job(&self) -> Option<common::Job> {
        let build_config = self.get_build_config()?;
        let run_config = self.get_run_config()?;
        let stop_config = self.get_stop_config()?;

        let steps = vec![
            common::Step::BuildImage(build_config),
            common::Step::StopContainer(stop_config),
            common::Step::RunContainer(run_config),
        ];

        Some(common::Job {
            needs: Vec::new(),
            steps,
        })
    }
}

mod raw {
    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};

    use crate::lib::{config, utils};

    use anyhow::anyhow;

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Services {
        #[serde(default)]
        services: HashMap<String, Service>,

        #[serde(default)]
        networks: HashMap<String, Network>,

        #[serde(default)]
        volumes: HashMap<String, Volume>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Service {
        #[serde(default = "default_global")]
        global: bool,
        build: Option<Build>,
        image: Option<String>,

        #[serde(default)]
        volumes: HashMap<String, String>,

        #[serde(default)]
        networks: Vec<String>,

        #[serde(default)]
        ports: Vec<String>,
        command: Option<Vec<String>>,
        restart: Option<String>,

        #[serde(default)]
        env: HashMap<String, String>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Build {
        path: String,
        dockerfile: Option<String>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Network {
        #[serde(default = "default_global")]
        global: bool,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
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

            let networks = config::utils::get_networks_names(context, self.networks)?;
            let volumes = config::utils::get_volumes_names(context, self.volumes)?;

            Ok(super::Service {
                image: get_image_name(context, self.image, self.global)?,
                container: get_container_name(context, self.global)?,
                command: self.command,
                ports: parse_port_mapping(self.ports)?,
                restart: self.restart.unwrap_or(String::from("on_failure")),
                env: config::utils::substitute_vars_dict(context, self.env)?,
                networks,
                volumes,
                build,
            })
        }
    }

    fn parse_port_mapping(
        ports: Vec<String>,
    ) -> Result<Vec<common::PortMapping>, config::LoadConfigError> {
        let res: Result<Vec<_>, anyhow::Error> = ports
            .into_iter()
            .map(|port| {
                let splits: Vec<&str> = port.split("/").collect();
                let (ports, proto) = if splits.len() == 1 {
                    (splits[0].to_string(), None)
                } else if splits.len() == 2 {
                    (splits[0].to_string(), Some(splits[1].to_string()))
                } else {
                    return Err(anyhow!("Invald port mapping: {}", port));
                };

                let splits: Vec<&str> = ports.split(":").collect();

                let (host, host_port, container_port) = if splits.len() == 2 {
                    (None, splits[0].parse()?, splits[1].parse()?)
                } else if splits.len() == 3 {
                    (
                        Some(splits[0].to_string()),
                        splits[1].parse()?,
                        splits[2].parse()?,
                    )
                } else {
                    return Err(anyhow!("Invalid port mapping: {}", port).into());
                };

                Ok(common::PortMapping {
                    container_port,
                    proto: proto.unwrap_or(String::from("tcp")),
                    host_port,
                    host,
                })
            })
            .collect();
        Ok(res?)
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
            let path = utils::try_expand_home(config::utils::substitute_vars(context, self.path)?);
            Ok(super::Build {
                path,
                dockerfile: self.dockerfile,
            })
        }
    }

    pub async fn load<'a>(
        context: &config::LoadContext<'a>,
    ) -> Result<super::Services, super::LoadConfigError> {
        let path = context.project_root()?.join(super::SERVICES_CONFIG);
        if !path.exists() {
            return Ok(Default::default());
        }
        config::load_sync::<Services>(path, context).await
    }
}
