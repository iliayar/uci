use std::{collections::HashMap, path::PathBuf};

use anyhow::anyhow;

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
    pub async fn load<'a>(context: &super::State<'a>) -> Result<Services, anyhow::Error> {
        raw::load(context).await
    }

    pub fn get(&self, service: &str) -> Option<&Service> {
        self.services.get(service)
    }

    pub fn get_network_name<S: AsRef<str>>(
        &self,
        project_id: S,
        network: String,
    ) -> Result<String, anyhow::Error> {
        let global = self
            .networks
            .get(&network)
            .ok_or_else(|| anyhow!("No such network {}", network))?
            .global;
        Ok(get_resource_name(project_id.as_ref(), network, global))
    }

    pub fn get_volume_name<S: AsRef<str>>(
        &self,
        project_id: S,
        volume: String,
    ) -> Result<String, anyhow::Error> {
        if let Some(v) = self.volumes.get(&volume) {
            Ok(get_resource_name(project_id.as_ref(), volume, v.global))
        } else {
            Ok(volume)
        }
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
            name: self.container.clone(),
            image: self.image.clone(),
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

    use crate::{config, utils};

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

        fn load_raw(self, state: &config::State) -> Result<Self::Output, anyhow::Error> {
            let mut res = super::Services {
                networks: self.networks.load_raw(state)?,
                volumes: self.volumes.load_raw(state)?,
                ..Default::default()
            };

            let services: Result<HashMap<_, _>, anyhow::Error> = {
                let mut state = state.clone();
                state.set(&res);
                self.services
                    .into_iter()
                    .map(|(id, service)| {
                        let mut state = state.clone();
                        state.set_named("service_id", &id);
                        let service = service.load_raw(&state)?;
                        Ok((id, service))
                    })
                    .collect()
            };

            res.services = services?;
            Ok(res)
        }
    }

    impl config::LoadRawSync for Network {
        type Output = super::Network;

        fn load_raw(self, context: &config::State) -> Result<Self::Output, anyhow::Error> {
            Ok(super::Network {
                global: self.global,
            })
        }
    }

    impl config::LoadRawSync for Volume {
        type Output = super::Volume;

        fn load_raw(self, context: &config::State) -> Result<Self::Output, anyhow::Error> {
            Ok(super::Volume {
                global: self.global,
            })
        }
    }

    impl config::LoadRawSync for Service {
        type Output = super::Service;

        fn load_raw(self, context: &config::State) -> Result<Self::Output, anyhow::Error> {
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
                restart: self.restart.unwrap_or_else(|| String::from("on_failure")),
                env: config::utils::substitute_vars_dict(context, self.env)?,
                networks,
                volumes,
                build,
            })
        }
    }

    fn parse_port_mapping(ports: Vec<String>) -> Result<Vec<common::PortMapping>, anyhow::Error> {
        let res: Result<Vec<_>, anyhow::Error> = ports
            .into_iter()
            .map(|port| {
                let splits: Vec<&str> = port.split('/').collect();
                let (ports, proto) = if splits.len() == 1 {
                    (splits[0].to_string(), None)
                } else if splits.len() == 2 {
                    (splits[0].to_string(), Some(splits[1].to_string()))
                } else {
                    return Err(anyhow!("Invald port mapping: {}", port));
                };

                let splits: Vec<&str> = ports.split(':').collect();

                let (host, host_port, container_port) = if splits.len() == 2 {
                    (None, splits[0].parse()?, splits[1].parse()?)
                } else if splits.len() == 3 {
                    (
                        Some(splits[0].to_string()),
                        splits[1].parse()?,
                        splits[2].parse()?,
                    )
                } else {
                    return Err(anyhow!("Invalid port mapping: {}", port));
                };

                Ok(common::PortMapping {
                    container_port,
                    proto: proto.unwrap_or_else(|| String::from("tcp")),
                    host_port,
                    host,
                })
            })
            .collect();
        res
    }

    fn get_image_name(
        context: &config::State,
        image: Option<String>,
        global: bool,
    ) -> Result<String, anyhow::Error> {
        let service_id = context.get_named("service_id").cloned()?;
        if let Some(image) = image {
            // Will pull specified image
            Ok(image)
        } else if global {
            // Image name is service name
            Ok(service_id)
        } else {
            // Image name is scoped under project
            let project_info: &config::ProjectInfo = context.get()?;
            Ok(format!("{}_{}", project_info.id, service_id))
        }
    }

    fn get_container_name(context: &config::State, global: bool) -> Result<String, anyhow::Error> {
        let service_id = context.get_named("service_id").cloned()?;
        if global {
            // Container name is service name
            Ok(service_id)
        } else {
            // Container name is scoped under project
            let project_info: &config::ProjectInfo = context.get()?;
            Ok(format!("{}_{}", project_info.id, service_id))
        }
    }

    impl config::LoadRawSync for Build {
        type Output = super::Build;

        fn load_raw(self, context: &config::State) -> Result<Self::Output, anyhow::Error> {
            let path = utils::try_expand_home(config::utils::substitute_vars(context, self.path)?);
            Ok(super::Build {
                path,
                dockerfile: self.dockerfile,
            })
        }
    }

    pub async fn load<'a>(context: &config::State<'a>) -> Result<super::Services, anyhow::Error> {
        let project_info: &config::ProjectInfo = context.get()?;
        let path = project_info.path.join(super::SERVICES_CONFIG);
        if !path.exists() {
            return Ok(Default::default());
        }
        config::load_sync::<Services>(path.clone(), context)
            .await
            .map_err(|err| anyhow::anyhow!("Failed to load services from {:?}: {}", path, err))
    }
}

fn get_resource_name(project_id: &str, name: String, global: bool) -> String {
    if global {
        name
    } else {
        format!("{}_{}", project_id, name)
    }
}
