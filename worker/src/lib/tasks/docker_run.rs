use super::error::TaskError;
use crate::lib::docker;
use bollard::container::{Config, CreateContainerOptions};
use common::RunContainerConfig;
use log::*;

pub async fn docker_run(
    docker: &docker::Docker,
    config: RunContainerConfig,
) -> Result<(), TaskError> {
    if let Ok(_) = docker.con.inspect_container(&config.name, None).await {
        warn!(
            "Container with name {} alredy exists. Trying to remove",
            config.name
        );

        docker.con.remove_container(&config.name, None).await?;
    }

    let name = docker
        .con
        .create_container(
            Some(CreateContainerOptions {
                name: config.name,
                ..CreateContainerOptions::default()
            }),
            Config {
                image: Some(config.image),
                ..Config::default()
            },
        )
        .await?
        .id;
    info!("Created container '{}'", name);

    docker.con.start_container::<&str>(&name, None).await?;
    info!("Container started '{}'", name);

    Ok(())
}
