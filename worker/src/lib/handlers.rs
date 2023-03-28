use std::convert::Infallible;

use warp::hyper::StatusCode;

use super::docker;
use super::executor::{Executor, ExecutorError};
use super::context::Context;
use common::Pipeline;
use log::*;

pub async fn run(context: Context, config: Pipeline) -> Result<impl warp::Reply, Infallible> {
    match run_impl(context, config).await {
        Ok(_) => Ok(StatusCode::OK),
        Err(err) => {
            error!("Executor error: {}", err);
            Ok(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn run_impl(context: Context, config: Pipeline) -> Result<(), ExecutorError> {
    debug!("Running with config {:?}", config);

    let executor = Executor::new(context)?;
    tokio::spawn(executor.run(config));

    info!("Executor started");
    Ok(())
}
