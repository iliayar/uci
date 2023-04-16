use std::convert::Infallible;

use warp::hyper::StatusCode;

use common::Pipeline;
use log::*;
use worker_lib::executor::Executor;

pub async fn run(
    deps: super::filters::Deps,
    config: Pipeline,
) -> Result<impl warp::Reply, Infallible> {
    match run_impl(deps, config).await {
        Ok(_) => Ok(StatusCode::OK),
        Err(err) => {
            error!("Executor error: {}", err);
            Ok(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn run_impl(deps: super::filters::Deps, config: Pipeline) -> Result<(), anyhow::Error> {
    tokio::spawn(async move {
        let state = deps.state.as_ref();
        match state.get::<Executor>() {
            Ok(executor) => {
                executor.run(deps.state.as_ref(), config).await;
            }
            Err(err) => {
                error!("Failed to run pipeline: {}", err);
            }
        }
    });

    info!("Executor started");
    Ok(())
}
