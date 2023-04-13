use std::convert::Infallible;

use warp::hyper::StatusCode;

use common::Pipeline;
use log::*;
use worker_lib::context::Context;
use worker_lib::executor::{Executor, ExecutorError};

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
    let executor = Executor::new(context)?;
    tokio::spawn(executor.run(config));

    info!("Executor started");
    Ok(())
}
