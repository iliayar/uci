use std::{collections::HashMap, path::PathBuf};

use bollard::{
    container::{self, CreateContainerOptions, LogsOptions, RemoveContainerOptions},
    exec::{CreateExecOptions, StartExecResults},
    image::{BuildImageOptions, CreateImageOptions},
    models::HostConfig,
    network::{ConnectNetworkOptions, CreateNetworkOptions},
    volume::CreateVolumeOptions,
};

use anyhow::anyhow;
use common::state::State;
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
pub struct LogsParams {
    container: String,
    follow: bool,
    tail: Option<usize>,
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

    #[builder(default = "Default::default()")]
    env: HashMap<String, String>,

    hostname: Option<String>,
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

    #[builder(default = "Default::default()")]
    env: HashMap<String, String>,
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

pub enum ContainerStatus {
    Running,
    NotRunning,
    Starting,
    Restarting,
    Dead,
    Exited(i64),
    Unknown,
}

impl Docker {
    pub fn init() -> Result<Docker, DockerError> {
        let docker = bollard::Docker::connect_with_socket_defaults()?;

        Ok(Docker { con: docker })
    }

    pub async fn status(&self, name: impl AsRef<str>) -> Result<ContainerStatus, DockerError> {
        if let Ok(info) = self.con.inspect_container(name.as_ref(), None).await {
            if let Some(state) = info.state {
                if let Some(status) = state.status {
                    let status = match status {
                        bollard::models::ContainerStateStatusEnum::EMPTY => {
                            ContainerStatus::Unknown
                        }
                        bollard::models::ContainerStateStatusEnum::CREATED => {
                            ContainerStatus::Starting
                        }
                        bollard::models::ContainerStateStatusEnum::RUNNING => {
                            ContainerStatus::Running
                        }
                        bollard::models::ContainerStateStatusEnum::PAUSED => {
                            ContainerStatus::NotRunning
                        }
                        bollard::models::ContainerStateStatusEnum::RESTARTING => {
                            ContainerStatus::Restarting
                        }
                        bollard::models::ContainerStateStatusEnum::REMOVING => {
                            ContainerStatus::NotRunning
                        }
                        bollard::models::ContainerStateStatusEnum::EXITED => {
                            // FIXME: Too lazy to make it optional. Why 1?
                            ContainerStatus::Exited(state.exit_code.unwrap_or(1))
                        }
                        bollard::models::ContainerStateStatusEnum::DEAD => ContainerStatus::Dead,
                    };
                    return Ok(status);
                }

                Ok(ContainerStatus::Unknown)
            } else {
                Ok(ContainerStatus::Unknown)
            }
        } else {
            Ok(ContainerStatus::NotRunning)
        }
    }

    pub async fn pull<'a>(&self, state: &State<'a>, params: PullParams) -> Result<(), DockerError> {
        info!("Pulling image {} done", params.image);
        let mut logger = super::executor::Logger::new(state).await?;

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
                if !status.is_empty() {
                    logger.regular(status).await?;
                }
            }
            if let Some(error) = result.error {
                logger.error(error).await?;
            }
            if let Some(progress) = result.progress {
                if !progress.is_empty() {
                    logger.regular(progress).await?;
                }
            }
        }

        info!("Pulling image {} done", params.image);

        Ok(())
    }

    pub async fn build<'a>(
        &self,
        state: &State<'a>,
        params: BuildParams,
    ) -> Result<(), DockerError> {
        let mut logger = super::executor::Logger::new(state).await?;
        let body = file_utils::open_async_stream(params.tar_path).await?;

        let pipeline_run: Option<&super::executor::PipelineRun> = state.get().ok();

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

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

        info!("Building image {} done", params.image);
        loop {
            #[rustfmt::skip]
            let result = tokio::select! {
                result = results.next() => result,
                _ = interval.tick() => {
		    if let Some(pipeline_run) = pipeline_run {
		        if pipeline_run.canceled().await {
                            break;
		        }
		    }
		    continue;
                }
            };

            let result = if let Some(result) = result {
                result?
            } else {
                break;
            };

            if let Some(stream) = result.stream {
                if !stream.is_empty() {
                    logger.regular(stream).await?;
                }
            }
            if let Some(error) = result.error {
                logger.error(error).await?;
            }
        }
        info!("Building image {} done", params.image);

        Ok(())
    }

    pub fn logs<'a>(
        &self,
        params: LogsParams,
    ) -> impl futures::Stream<Item = Result<models::PipelineMessage, anyhow::Error>> {
        let mut logs = self.con.logs(
            &params.container,
            Some(LogsOptions {
                follow: params.follow,
                stdout: true,
                stderr: true,
                timestamps: true,
                tail: params
                    .tail
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "all".to_string()),
                ..Default::default()
            }),
        );

        #[rustfmt::skip]
        async_stream::try_stream! {
            while let Some(log) = logs.next().await {
                yield make_pipeline_log(params.container.clone(), log?)?;
            }
        }
    }

    pub async fn create_container<'a>(
        &self,
        state: &State<'a>,
        params: CreateContainerParams,
    ) -> Result<String, DockerError> {
        let mut logger = super::executor::Logger::new(state).await?;

        if let Some(name) = params.name.as_ref() {
            if let Ok(container) = self.con.inspect_container(name, None).await {
                if let Some(id) = container.id {
                    logger
                        .warning(format!("Container {} already exists. Skip creating", name))
                        .await?;
                    return Ok(id);
                }
            }
        }

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
            env: Some(get_env(params.env)),
	    hostname: params.hostname,
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

        logger
            .regular(format!("Created container {}", name))
            .await?;
        info!("Created container {}", name);

        Ok(name)
    }

    pub async fn start_container<'a>(
        &self,
        state: &State<'a>,
        params: StartContainerParams,
    ) -> Result<(), DockerError> {
        let mut logger = super::executor::Logger::new(state).await?;

        logger
            .regular(format!("Starting container {}", params.name))
            .await?;

        self.con
            .start_container::<&str>(&params.name, None)
            .await
            .map_err(Into::into)
    }

    pub async fn run_command<'a>(
        &self,
        state: &State<'a>,
        params: RunCommandParams,
    ) -> Result<(), DockerError> {
        let mut logger = super::executor::Logger::new(state).await?;

        let pipeline_run: Option<&super::executor::PipelineRun> = state.get().ok();

        let host_config = HostConfig {
            binds: Some(binds_from_map(params.mounts)),
            ..Default::default()
        };

        let config = container::Config {
            image: Some(params.image),
            tty: Some(true),
            host_config: Some(host_config),
            env: Some(get_env(params.env)),
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

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

        if let StartExecResults::Attached { mut output, .. } =
            self.con.start_exec(&exec, None).await?
        {
            loop {
                #[rustfmt::skip]
                let msg = tokio::select! {
                    msg = output.next() => msg,
                    _ = interval.tick() => {
			if let Some(pipeline_run) = pipeline_run {
			    if pipeline_run.canceled().await {
				break;
			    }
			}
			continue;
                    }
                };

                let msg = if let Some(msg) = msg {
                    msg?
                } else {
                    break;
                };

                match msg {
                    container::LogOutput::StdErr { message } => {
                        let bytes: Vec<u8> = message.into_iter().collect();
                        logger
                            .error(String::from_utf8_lossy(&bytes).to_string())
                            .await?;
                    }
                    container::LogOutput::StdOut { message }
                    | container::LogOutput::StdIn { message }
                    | container::LogOutput::Console { message } => {
                        let bytes: Vec<u8> = message.into_iter().collect();
                        logger
                            .regular(String::from_utf8_lossy(&bytes).to_string())
                            .await?;
                    }
                }
            }
        } else {
            unreachable!();
        }

        let exec_result = self.con.inspect_exec(&exec).await?;

        self.con
            .remove_container(
                &name,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await?;

        match exec_result.exit_code {
            None => {
                info!("Container done '{}' with no status code", exec,);
            }
            Some(status) => {
                info!("Container done '{}' with status code: {}", exec, status,);
                if status != 0 {
                    return Err(anyhow!("Script exited with status code {}", status).into());
                }
            }
        }

        Ok(())
    }

    pub async fn stop_container<'a>(
        &self,
        state: &State<'a>,
        stop_params: StopContainerParams,
    ) -> Result<(), DockerError> {
        let mut logger = super::executor::Logger::new(state).await?;

        let params = match self.con.inspect_container(&stop_params.name, None).await {
            Ok(params) => params,
            Err(err) => {
                logger
                    .warning(format!(
                        "Cannot inspect container: {}. Assuming doesn't exists",
                        err
                    ))
                    .await?;
                error!("Cannot inspect container: {}. Assuming doesn't exists", err);
                return Ok(());
            }
        };

        if let Some(state) = params.state {
            if let Some(true) = state.running {
                logger
                    .regular(format!("Stopping container {}", stop_params.name))
                    .await?;
                self.con.stop_container(&stop_params.name, None).await?;
            } else {
                logger
                    .warning("Cannot get container runnig state, do not stop".to_string())
                    .await?;
                warn!("Cannot get container runnig state, do not stop");
            }
        } else {
            logger
                .warning("Cannot get container state, do not stop".to_string())
                .await?;
            warn!("Cannot get container state, do not stop");
        }

        logger
            .regular(format!("Container stopped {}, removing", stop_params.name))
            .await?;

        self.con.remove_container(&stop_params.name, None).await?;

        Ok(())
    }

    pub async fn get_workdir(&self, image: &str) -> Result<PathBuf, DockerError> {
        let conf = self.con.inspect_image(image).await?;
        let workdir = conf
            .config
            .ok_or_else(|| anyhow!("Image {} has no 'Config'", image))?
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
        if self.con.inspect_network::<&str>(name, None).await.is_ok() {
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
        if self.con.inspect_volume(name).await.is_ok() {
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

fn get_env(env: HashMap<String, String>) -> Vec<String> {
    env.into_iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect()
}

fn make_pipeline_log(
    container: String,
    log: bollard::container::LogOutput,
) -> Result<models::PipelineMessage, anyhow::Error> {
    let (t, text) = match log {
        container::LogOutput::StdErr { message } => {
            let bytes: Vec<u8> = message.into_iter().collect();
            (
                models::LogType::Error,
                String::from_utf8_lossy(&bytes).to_string(),
            )
        }
        container::LogOutput::StdOut { message }
        | container::LogOutput::StdIn { message }
        | container::LogOutput::Console { message } => {
            let bytes: Vec<u8> = message.into_iter().collect();
            (
                models::LogType::Regular,
                String::from_utf8_lossy(&bytes).to_string(),
            )
        }
    };

    let (timestamp, text) = text
        .split_once(' ')
        .ok_or_else(|| anyhow!("No timestamp in docker log output"))?;

    let timestamp = chrono::DateTime::parse_from_rfc3339(timestamp)
        .map_err(|err| anyhow!("Failed to parse docker timestamp in log: {}", err))?;

    let log = models::PipelineMessage::ContainerLog {
        container,
        t,
        text: text.to_string(),
        timestamp: timestamp.into(),
    };

    Ok(log)
}
