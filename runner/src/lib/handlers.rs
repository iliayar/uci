use std::convert::Infallible;

use log::*;
use warp::hyper::StatusCode;

use super::{context::Context, filters::ContextStore};

pub async fn run(
    project_id: String,
    action_id: String,
    store: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> Result<impl warp::Reply, Infallible> {
    info!("Running hook {} for project {}", action_id, project_id);
    let config = store.context().config().await;

    // TODO: Respond with error messages
    if !config.has_project_action(&project_id, &action_id).await {
        return Ok(StatusCode::NOT_FOUND);
    }

    tokio::spawn(async move {
        if let Err(err) = config.run_project_action(worker_context, &project_id, &action_id).await {
            error!(
                "Failed to execute action {} on project {}: {}",
                action_id, project_id, err
            );
        }
    });

    Ok(StatusCode::OK)
}

pub async fn reload_config(store: ContextStore) -> Result<impl warp::Reply, Infallible> {
    // TODO: Respond with error messages
    match store.context().reload_config().await {
        Ok(_) => Ok(StatusCode::OK),
        Err(err) => {
            error!("Failed to reload config: {}", err);
            Ok(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
