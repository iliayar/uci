use std::{collections::HashMap, path::PathBuf};

use anyhow::anyhow;
use common::state::State;

#[derive(Debug, Default)]
pub struct Services {
    services: HashMap<String, Service>,
    pub networks: HashMap<String, String>,
    pub volumes: HashMap<String, String>,
}

#[derive(Debug)]
pub struct Service {
    id: String,
    container: String,
    build: Option<Build>,
    image: String,
    hostname: Option<String>,
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

pub struct ServicesDescription {
    pub services: Vec<ServiceDescription>,
}

pub struct ServiceDescription {
    pub name: String,
    pub status: worker_lib::docker::ContainerStatus,
}

impl Services {
    pub fn merge(self, other: Services) -> Result<Services, anyhow::Error> {
        let mut services = HashMap::new();

        for (id, service) in self.services.into_iter().chain(other.services.into_iter()) {
            if services.contains_key(&id) {
                return Err(anyhow!("Service {} duplicates", id));
            }
            services.insert(id, service);
        }

        let mut networks = HashMap::new();

        for (id, network) in self.networks.into_iter().chain(other.networks.into_iter()) {
            if networks.contains_key(&id) {
                return Err(anyhow!("Network {} duplicates", id));
            }
            networks.insert(id, network);
        }

        let mut volumes = HashMap::new();

        for (id, volume) in self.volumes.into_iter().chain(other.volumes.into_iter()) {
            if volumes.contains_key(&id) {
                return Err(anyhow!("Volume {} duplicates", id));
            }
            volumes.insert(id, volume);
        }

        return Ok(Services {
            services,
            networks,
            volumes,
        });
    }

    pub fn get(&self, service: &str) -> Option<&Service> {
        self.services.get(service)
    }

    pub async fn list_services<'a>(
        &self,
        state: &State<'a>,
    ) -> Result<ServicesDescription, anyhow::Error> {
        let mut services = Vec::new();
        for (service_id, service) in self.services.iter() {
            services.push(service.describe(state).await?);
        }
        Ok(ServicesDescription { services })
    }
}

impl Service {
    pub async fn describe<'a>(
        &self,
        state: &State<'a>,
    ) -> Result<ServiceDescription, anyhow::Error> {
        let docker: &worker_lib::docker::Docker = state.get()?;
        let status = docker.status(&self.container).await?;

        Ok(ServiceDescription {
            name: self.id.clone(),
            status,
        })
    }

    pub fn get_start_job(&self, build: bool) -> Option<common::Job> {
        let mut steps = Vec::new();

        if build {
            steps.push(common::Step::BuildImage(self.get_build_config()?));
        }

        steps.push(common::Step::RunContainer(self.get_run_config()?));

        let job = common::Job {
            enabled: true,
            needs: vec![],
            stage: None,
            steps,
        };

        Some(job)
    }

    pub fn get_stop_job(&self) -> Option<common::Job> {
        let steps = vec![common::Step::StopContainer(self.get_stop_config()?)];

        let job = common::Job {
            enabled: true,
            needs: vec![],
            stage: None,
            steps,
        };

        Some(job)
    }

    pub fn get_restart_job(&self, build: bool) -> Option<common::Job> {
        let mut steps = Vec::new();
        if build {
            steps.push(common::Step::BuildImage(self.get_build_config()?));
        }
        steps.push(common::Step::StopContainer(self.get_stop_config()?));
        steps.push(common::Step::RunContainer(self.get_run_config()?));

        let job = common::Job {
            enabled: true,
            needs: vec![],
            stage: None,
            steps,
        };

        Some(job)
    }

    pub fn get_logs_job(&self, follow: bool, tail: Option<usize>) -> Option<common::Job> {
        let config = common::ServiceLogsConfig {
            container: self.container.clone(),
            follow,
            tail,
        };

        let steps = vec![common::Step::ServiceLogs(config)];

        let job = common::Job {
            enabled: true,
            needs: vec![],
            stage: None,
            steps,
        };

        Some(job)
    }

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
            hostname: self.hostname.clone(),
            volumes,
            networks,
        })
    }

    pub fn get_stop_config(&self) -> Option<common::StopContainerConfig> {
        Some(common::StopContainerConfig {
            name: self.container.clone(),
        })
    }

    pub fn logs<'a>(
        &self,
        state: &State<'a>,
        follow: bool,
        tail: Option<usize>,
    ) -> Result<
        impl futures::Stream<Item = Result<models::PipelineMessage, anyhow::Error>>,
        anyhow::Error,
    > {
        let docker: &worker_lib::docker::Docker = state.get()?;
        let mut params = worker_lib::docker::LogsParamsBuilder::default();
        params
            .container(self.container.clone())
            .follow(follow)
            .tail(tail);

        Ok(docker.logs(
            params
                .build()
                .map_err(|e| anyhow!("Invalid stop container params: {}", e))?,
        ))
    }
}

pub use dyn_obj::DynServices;

mod dyn_obj {
    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize)]
    pub struct DynServices {
        pub networks: HashMap<String, String>,
        pub volumes: HashMap<String, String>,
    }

    impl From<&super::Services> for DynServices {
        fn from(services: &super::Services) -> Self {
            Self {
                networks: services.networks.clone(),
                volumes: services.volumes.clone(),
            }
        }
    }
}

pub mod raw {
    use crate::config;

    use dynconf::*;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    use anyhow::{anyhow, Result};

    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct Services {
        #[serde(default)]
        services: HashMap<String, Service>,

        #[serde(default)]
        networks: HashMap<String, Network>,

        #[serde(default)]
        volumes: HashMap<String, Volume>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    struct Service {
        global: Option<bool>,
        build: Option<Build>,
        image: Option<String>,

        volumes: Option<HashMap<String, util::DynString>>,
        networks: Option<Vec<String>>,

        #[serde(default)]
        ports: Vec<String>,
        command: Option<Vec<String>>,
        restart: Option<String>,

        #[serde(default)]
        env: HashMap<String, util::DynString>,

        hostname: Option<String>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    struct Build {
        path: util::DynPath,
        dockerfile: Option<String>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    struct Network {
        global: Option<bool>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    struct Volume {
        global: Option<bool>,
    }

    #[async_trait::async_trait]
    impl util::DynValue for Services {
        type Target = super::Services;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            let networks = self.networks.load(state).await?;
            let volumes = self.volumes.load(state).await?;

            state.mutate_global(config::utils::wrap_dyn_f(|mut dynobj| {
                dynobj.services = Some(super::DynServices {
                    networks: networks.clone(),
                    volumes: volumes.clone(),
                });
                Ok(dynobj)
            }))?;

            let services = self.services.load(state).await?;

            Ok(super::Services {
                networks,
                volumes,
                services,
            })
        }
    }

    #[async_trait::async_trait]
    impl util::DynValue for Network {
        type Target = String;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            let dynobj = config::utils::get_dyn_object(state)?;
            let id = dynobj._id.ok_or_else(|| anyhow!("No _id binding"))?;
            let project_id = dynobj
                .project
                .ok_or_else(|| anyhow!("No project binding"))?
                .id;

            Ok(super::get_resource_name(
                project_id,
                id,
                self.global.unwrap_or(false),
            ))
        }
    }

    #[async_trait::async_trait]
    impl util::DynValue for Volume {
        type Target = String;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            let dynobj = config::utils::get_dyn_object(state)?;
            let id = dynobj._id.ok_or_else(|| anyhow!("No _id binding"))?;
            let project_id = dynobj
                .project
                .ok_or_else(|| anyhow!("No project binding"))?
                .id;

            Ok(super::get_resource_name(
                project_id,
                id,
                self.global.unwrap_or(false),
            ))
        }
    }

    #[async_trait::async_trait]
    impl util::DynValue for Service {
        type Target = super::Service;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            let dynobj = config::utils::get_dyn_object(state)?;
            let service_id = dynobj._id.ok_or_else(|| anyhow!("No _id binding"))?;
            let services = dynobj
                .services
                .ok_or_else(|| anyhow!("No services binding"))?;
            let project_id = dynobj
                .project
                .ok_or_else(|| anyhow!("No project binding"))?
                .id;

            let build = if let Some(build) = self.build {
                Some(build.load(state).await?)
            } else {
                None
            };

            let networks: Result<Vec<String>> = self
                .networks
                .unwrap_or_default()
                .into_iter()
                .map(|name: String| {
                    services
                        .networks
                        .get(&name)
                        .ok_or_else(|| anyhow!("Unknown network: {}", name))
                        .cloned()
                })
                .collect();

            let mut volumes: HashMap<String, String> = Default::default();
            for (key, name) in self.volumes.unwrap_or_default().into_iter() {
                let name = name.load(state).await?;
                let name = if let Some(name) = services.volumes.get(&name) {
                    name.clone()
                } else {
                    // name is path
                    name
                };
                volumes.insert(key, name);
            }

            let global = self.global.unwrap_or(false);

            let image = if let Some(image) = self.image {
                // Will pull specified image
                image
            } else if global {
                // Image name is service name
                service_id.clone()
            } else {
                // Image name is scoped under project
                format!("{}_{}", project_id, service_id)
            };

            let container = if global {
                // Container name is service name
                service_id.clone()
            } else {
                // Container name is scoped under project
                format!("{}_{}", project_id, service_id)
            };

            Ok(super::Service {
                id: service_id,
                command: self.command,
                ports: parse_port_mapping(self.ports)?,
                restart: self.restart.unwrap_or_else(|| String::from("on_failure")),
                env: self.env.load(state).await?,
                hostname: self.hostname,
                networks: networks?,
                volumes,
                container,
                image,
                build,
            })
        }
    }

    #[async_trait::async_trait]
    impl util::DynValue for Build {
        type Target = super::Build;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            Ok(super::Build {
                path: self.path.load(state).await?,
                dockerfile: self.dockerfile,
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
}

fn get_resource_name(project_id: impl AsRef<str>, name: String, global: bool) -> String {
    if global {
        name
    } else {
        format!("{}_{}", project_id.as_ref(), name)
    }
}
