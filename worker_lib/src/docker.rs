use std::{collections::HashMap, path::PathBuf};

use bollard::{
    container::{self, CreateContainerOptions, RemoveContainerOptions, WaitContainerOptions},
    exec::{CreateExecOptions, StartExecResults},
    image::{BuildImageOptions, CreateImageOptions},
    models::{ContainerState, HostConfig},
    network::{ConnectNetworkOptions, CreateNetworkOptions},
    volume::CreateVolumeOptions,
};

use anyhow::anyhow;
use log::*;

use futures::StreamExt;

use crate::utils::file_utils;

#[derive(Debug, thiserror::Error)]
pub enum DockerError {
    #[error(transparent)]
    InternalDockerError(#[from] bollard::errors::Error),

    #[error(transparent)]
    InternalIoError(#[from] tokio::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Clone)]
pub struct Docker {
    con: bollard::Docker,
}

#[derive(derive_builder::Builder)]
pub struct PullParams {
    image: String,

    #[builder(default = "default_tag()")]
    tag: String,
}

#[derive(derive_builder::Builder)]
pub struct BuildParams {
    tar_path: PathBuf,
    image: String,

    #[builder(default = "default_tag()")]
    tag: String,

    #[builder(default = "default_dockerfile()")]
    dockerfile: String,
}

#[derive(derive_builder::Builder)]
pub struct CreateContainerParams {
    image: String,
    name: Option<String>,

    #[builder(default = "Default::default()")]
    mounts: HashMap<String, String>,

    #[builder(default = "Default::default()")]
    networks: Vec<String>,

    #[builder(default = "Default::default()")]
    ports: Vec<common::PortMapping>,

    command: Option<Vec<String>>,

    #[builder(default = "default_restart()")]
    restart: String,
}

#[derive(derive_builder::Builder)]
pub struct StartContainerParams {
    name: String,
}

#[derive(derive_builder::Builder)]
pub struct StopContainerParams {
    name: String,
}

#[derive(derive_builder::Builder)]
pub struct RunCommandParams {
    image: String,
    #[builder(default = "Default::default()")]
    mounts: HashMap<String, String>,
    command: Vec<String>,
    workdir: Option<String>,
    #[builder(default = "Default::default()")]
    networks: Vec<String>,
}

pub enum DeployBuildParams {
    Build(BuildParams),
    Pull(PullParams),
}

fn default_tag() -> String {
    String::from("latest")
}

fn default_dockerfile() -> String {
    String::from("Dockerfile")
}

fn default_restart() -> String {
    String::from("on_failure")
}

impl Docker {
    pub fn init() -> Result<Docker, DockerError> {
        let docker = bollard::Docker::connect_with_socket_defaults()?;

        Ok(Docker { con: docker })
    }

    pub async fn pull(&self, params: PullParams) -> Result<(), DockerError> {
        info!("Pulling image {} done", params.image);

        let mut results = self.con.create_image::<&str>(
            Some(CreateImageOptions {
                from_image: &params.image,
                tag: &params.tag,
                ..Default::default()
            }),
            None,
            None,
        );

        while let Some(result) = results.next().await {
            let result = result?;

            if let Some(status) = result.status {
                info!("{}", status);
            }
            if let Some(error) = result.error {
                error!("{}", error);
            }
            if let Some(progress) = result.progress {
                info!("{}", progress);
            }
        }

        info!("Pulling image {} done", params.image);

        Ok(())
    }

    pub async fn build(&self, params: BuildParams) -> Result<(), DockerError> {
        let body = file_utils::open_async_stream(params.tar_path).await?;

        let tag = format!("{}:{}", params.image, params.tag);
        let mut results = self.con.build_image::<&str>(
            BuildImageOptions {
                dockerfile: &params.dockerfile,
                t: &tag,
                ..Default::default()
            },
            None,
            Some(body),
        );

        info!("Building image {} done", params.image);
        while let Some(result) = results.next().await {
            let result = result?;

            if let Some(status) = result.status {
                info!("{}", status);
            }
            if let Some(stream) = result.stream {
                info!("{}", stream);
            }
            if let Some(progress) = result.progress {
                info!("{}", progress);
            }
            if let Some(error) = result.error {
                error!("{}", error);
            }
        }
        info!("Building image {} done", params.image);

        Ok(())
    }

    pub async fn create_container(
        &self,
        params: CreateContainerParams,
    ) -> Result<String, DockerError> {
        let exposed_ports: HashMap<_, _> = params
            .ports
            .iter()
            .map(|ports| {
                (
                    format!("{}/{}", ports.container_port, ports.proto),
                    HashMap::new(),
                )
            })
            .collect();

        let host_config = HostConfig {
            binds: Some(binds_from_map(params.mounts)),
            port_bindings: Some(port_mappig(params.ports)),
            restart_policy: Some(get_restart_policy(&params.restart)),
            ..Default::default()
        };

        let config = container::Config {
            image: Some(params.image),
            host_config: Some(host_config),
            cmd: params.command,
            exposed_ports: Some(exposed_ports),
            ..Default::default()
        };

        let create_container_options = params.name.map(|name| CreateContainerOptions {
            name,
            ..Default::default()
        });

        let name = self
            .con
            .create_container(create_container_options, config)
            .await?
            .id;

        for network in params.networks {
            self.con
                .connect_network::<&str>(
                    &network,
                    ConnectNetworkOptions {
                        container: &name,
                        ..Default::default()
                    },
                )
                .await?;
        }
        info!("Created container {}", name);

        Ok(name)
    }

    pub async fn start_container(&self, params: StartContainerParams) -> Result<(), DockerError> {
        info!("Starting container {}", params.name);
        self.con
            .start_container::<&str>(&params.name, None)
            .await
            .map_err(Into::into)
    }

    pub async fn run_command(&self, params: RunCommandParams) -> Result<(), DockerError> {
        let host_config = HostConfig {
            binds: Some(binds_from_map(params.mounts)),
            ..Default::default()
        };

        let config = container::Config {
            image: Some(params.image),
            tty: Some(true),
            host_config: Some(host_config),
            ..Default::default()
        };

        debug!("Creating docker container with config: {:?}", config);

        let name = self
            .con
            .create_container::<&str, String>(
                Some(CreateContainerOptions {
                    ..Default::default()
                }),
                config,
            )
            .await?
            .id;

        for network in params.networks {
            self.con
                .connect_network::<&str>(
                    &network,
                    ConnectNetworkOptions {
                        container: &name,
                        ..Default::default()
                    },
                )
                .await?;
        }
        info!("Created container '{}'", name);

        self.con.start_container::<&str>(&name, None).await?;
        info!("Container started '{}'", name);

        let exec = self
            .con
            .create_exec::<String>(
                &name,
                CreateExecOptions {
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    cmd: Some(params.command),
                    working_dir: params.workdir,
                    ..Default::default()
                },
            )
            .await?
            .id;

        if let StartExecResults::Attached { mut output, .. } =
            self.con.start_exec(&exec, None).await?
        {
            while let Some(Ok(msg)) = output.next().await {
                info!("{}", msg);
            }
        } else {
            unreachable!();
        }

        info!("Container done '{}'", exec);

        self.con
            .remove_container(
                &name,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await?;

        Ok(())
    }

    pub async fn stop_container(
        &self,
        stop_params: StopContainerParams,
    ) -> Result<(), DockerError> {
        let params = match self.con.inspect_container(&stop_params.name, None).await {
            Ok(params) => params,
            Err(err) => {
                error!("Cannot inspect container: {}", err);
                warn!("Assuming container doesn't exists, do not remove it");
                return Ok(());
            }
        };

        if let Some(state) = params.state {
            if let Some(true) = state.running {
                info!("Stopping container {}", stop_params.name);
                self.con.stop_container(&stop_params.name, None).await?;
            } else {
                error!("Cannot get container runnig state, do not stop");
            }
        } else {
            error!("Cannot get container state, do not stop");
        }

        info!("Container stopped {}, removing", stop_params.name);
        self.con.remove_container(&stop_params.name, None).await?;

        Ok(())
    }

    pub async fn get_workdir(&self, image: &str) -> Result<PathBuf, DockerError> {
        let conf = self.con.inspect_image(image).await?;
        let workdir = conf
            .config
            .ok_or(anyhow!("Image {} has no 'Config'", image))?
            .working_dir;

        if let Some(workdir) = workdir {
            let p = PathBuf::from(workdir);
            if p.is_absolute() {
                Ok(p)
            } else {
                Ok(PathBuf::from("/").join(p))
            }
        } else {
            Ok(PathBuf::from("/"))
        }
    }

    pub async fn create_network_if_missing(&self, name: &str) -> Result<(), DockerError> {
        if let Ok(_) = self.con.inspect_network::<&str>(name, None).await {
            info!("Network {} already exists", name);
            Ok(())
        } else {
            info!("Creating network {}", name);
            self.con
                .create_network(CreateNetworkOptions {
                    name,
                    ..Default::default()
                })
                .await?;

            Ok(())
        }
    }

    pub async fn create_volume_if_missing(&self, name: &str) -> Result<(), DockerError> {
        if let Ok(_) = self.con.inspect_volume(name).await {
            info!("Volume {} already exists", name);
            Ok(())
        } else {
            info!("Creating volume {}", name);
            self.con
                .create_volume(CreateVolumeOptions {
                    name,
                    ..Default::default()
                })
                .await?;

            Ok(())
        }
    }
}

fn binds_from_map(mounts: HashMap<String, String>) -> Vec<String> {
    let mut volumes = Vec::new();

    for (host_path, container_path) in mounts.into_iter() {
        volumes.push(format!("{}:{}", host_path, container_path));
    }

    volumes
}

fn port_mappig(mapping: Vec<common::PortMapping>) -> bollard::models::PortMap {
    let mut map = HashMap::new();

    for common::PortMapping {
        container_port,
        proto,
        host_port,
        host,
    } in mapping
    {
        let key = format!("{}/{}", container_port, proto);
        if !map.contains_key(&key) {
            map.insert(key.clone(), Some(Vec::new()));
        }
        map.get_mut(&key)
            .unwrap()
            .as_mut()
            .unwrap()
            .push(bollard::models::PortBinding {
                host_ip: host,
                host_port: Some(host_port.to_string()),
            })
    }

    map
}

fn get_restart_policy(policy: &str) -> bollard::models::RestartPolicy {
    let policy = match policy {
        "always" => Some(bollard::models::RestartPolicyNameEnum::ALWAYS),
        "on_failure" => Some(bollard::models::RestartPolicyNameEnum::ON_FAILURE),
        _ => {
            warn!("Unknown restart policy: {}", policy);
            None
        }
    };

    bollard::models::RestartPolicy {
        name: policy,
        ..Default::default()
    }
}
