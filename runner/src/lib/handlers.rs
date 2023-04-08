use std::convert::Infallible;

use log::*;
use warp::hyper::StatusCode;

use super::{context::Context, filters::ContextStore};

pub async fn call(
    project_id: String,
    trigger_id: String,
    store: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> Result<impl warp::Reply, Infallible> {
    info!("Running trigger {} for project {}", trigger_id, project_id);
    let config = store.context().config().await;

    let trigger = super::config::ActionEvent::DirectCall {
        project_id: project_id.clone(),
        trigger_id: trigger_id.clone(),
    };
    trigger_projects_impl(trigger, store, worker_context).await;

    Ok(StatusCode::OK)
}

pub async fn update_repo(
    repo: String,
    store: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> Result<impl warp::Reply, Infallible> {
    info!("Running repo {}", repo);
    let trigger = super::config::ActionEvent::UpdateRepos {
        repos: vec![repo],
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

    let trigger = super::config::ActionEvent::ConfigReloaded;
    trigger_projects_impl(trigger, store, worker_context).await;

    Ok(())
}

pub async fn trigger_projects_impl(
    trigger: super::config::ActionEvent,
    store: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) {
    tokio::spawn(async move {
        match trigger_projects_impl_result(trigger, store, worker_context).await {
            Result::Err(err) => {
                error!("Failed to match actions: {}", err);
            }
            Result::Ok(_) => {}
        }
    });
}

pub async fn trigger_projects_impl_result(
    trigger: super::config::ActionEvent,
    store: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> Result<(), anyhow::Error> {
    let mut matched = store
        .context()
        .config()
        .await
        .get_projects_actions(trigger)
        .await?;

    // FIXME: Do it separately
    if matched.reload_config || !matched.reload_projects.is_empty() {
        store.context().reload_config().await?;
        let new_matched = store
            .context()
            .config()
            .await
            .get_projects_actions(super::config::ActionEvent::ConfigReloaded)
            .await?;
        matched.merge(new_matched);
    }

    let mut new_matcheds = Vec::new();
    for project_id in matched.reload_projects.iter() {
        let new_matched = store
            .context()
            .config()
            .await
            .get_projects_actions(super::config::ActionEvent::ProjectReloaded {
                project_id: project_id.clone(),
            })
            .await?;
	new_matcheds.push(new_matched);
    }
    for new_matched in new_matcheds.into_iter() {
	matched.merge(new_matched);
    }

    store
        .context()
        .config()
        .await
        .run_project_actions(worker_context, matched)
        .await?;

    Ok(())
}
