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

    let trigger = super::config::ActionTrigger::DirectCall {
        project_id: project_id.clone(),
        action_id: action_id.clone(),
    };
    trigger_projects_impl(trigger, store, worker_context).await;

    Ok(StatusCode::OK)
}

pub async fn reload_config(
    store: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> Result<impl warp::Reply, Infallible> {
    match reload_config_impl(store, worker_context).await {
        Ok(_) => Ok(StatusCode::OK),
        Err(err) => {
            error!("Failed to reload config: {}", err);
            Ok(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn reload_config_impl(
    store: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> Result<(), anyhow::Error> {
    store.context().reload_config().await?;

    let trigger = super::config::ActionTrigger::ConfigReloaded;
    trigger_projects_impl(trigger, store, worker_context).await;

    Ok(())
}

pub async fn trigger_projects_impl(
    trigger: super::config::ActionTrigger,
    store: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) {
    tokio::spawn(async move {
        if let Err(err) = store
            .context()
            .config()
            .await
            .run_project_actions(worker_context, trigger)
            .await
        {
            error!("Failed to execute actions: {}", err);
        }
    });
}
