use std::convert::Infallible;

use warp::hyper::StatusCode;

use super::docker;
use super::executor::{Executor, ExecutorError};
use common::Config;
use log::*;

pub async fn run(docker: docker::Docker, config: Config) -> Result<impl warp::Reply, Infallible> {
    match run_impl(docker, config).await {
        Ok(_) => Ok(StatusCode::OK),
        Err(err) => {
            error!("Executor error: {}", err);
            Ok(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn run_impl(docker: docker::Docker, config: Config) -> Result<(), ExecutorError> {
    debug!("Running with config {:?}", config);

    let executor = Executor::new(docker)?;
    tokio::spawn(executor.run(config));

    info!("Executor started");
    Ok(())
}
