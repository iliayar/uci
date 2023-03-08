use std::convert::Infallible;

use bollard::container::{
    Config, CreateContainerOptions, ListContainersOptions, StartContainerOptions,
};
use log::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use warp::hyper::StatusCode;

#[derive(Error, Debug)]
pub enum DockerError {
    #[error("Docker socket error: {0}")]
    DockerError(#[from] bollard::errors::Error),
}

#[derive(Clone)]
pub struct Docker {
    con: bollard::Docker,
}

#[derive(Serialize, Deserialize)]
pub struct RunRequest {
    name: String,
    image: String,
}

#[derive(Serialize, Deserialize)]
pub struct ErrorResponse {
    code: u16,
    message: String,
}

impl From<DockerError> for ErrorResponse {
    fn from(err: DockerError) -> Self {
        ErrorResponse {
            code: 500,
            message: format!("{}", err),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct RunResponse {}

impl Docker {
    pub fn init() -> Result<Docker, DockerError> {
        let docker = bollard::Docker::connect_with_socket_defaults()?;

        Ok(Docker { con: docker })
    }

    async fn run(self, request: RunRequest) -> Result<RunResponse, DockerError> {
        if let Ok(_) = self.con.inspect_container(&request.name, None).await {
            warn!(
                "Container with name {} alredy exists. Trying to remove",
                request.name
            );

            self.con.remove_container(&request.name, None).await?;
        }

        let name = self
            .con
            .create_container(
                Some(CreateContainerOptions {
                    name: request.name,
                    ..CreateContainerOptions::default()
                }),
                Config {
                    image: Some(request.image),
                    ..Config::default()
                },
            )
            .await?
            .id;
        info!("Created container '{}'", name);

        self.con.start_container::<&str>(&name, None).await?;
        info!("Container started '{}'", name);

        Ok(RunResponse {})
    }
}

pub async fn run(docker: Docker, request: RunRequest) -> Result<impl warp::Reply, Infallible> {
    match docker.run(request).await {
        Ok(response) => Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::OK,
        )),
        Err(err) => {
            error!("Container run handler failed: {}", err);
            Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse::from(err)),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}
