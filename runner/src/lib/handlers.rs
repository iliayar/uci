use std::convert::Infallible;

use log::*;
use warp::hyper::StatusCode;

use crate::lib::config::ActionType;

use super::{
    config::ActionEvent,
    context::Context,
    filters::{CallContext, ContextStore},
};

pub async fn call(
    call_context: CallContext,
    project_id: String,
    trigger_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    info!("Running trigger {} for project {}", trigger_id, project_id);

    call_context
        .check_authorized(Some(&project_id), ActionType::Write)
        .await?;

    trigger_projects_impl(
        call_context,
        ActionEvent::DirectCall {
            project_id: project_id.clone(),
            trigger_id: trigger_id.clone(),
        },
    )
    .await;

    Ok(StatusCode::OK)
}

pub async fn update_repo(
    call_context: CallContext,
    repo: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    info!("Running repo {}", repo);
    trigger_projects_impl(call_context, ActionEvent::UpdateRepos { repos: vec![repo] }).await;
    Ok(StatusCode::OK)
}

pub async fn reload_config(call_context: CallContext) -> Result<impl warp::Reply, warp::Rejection> {
    call_context
        .check_authorized::<&str>(None, super::config::ActionType::Write)
        .await?;

    match reload_config_impl(call_context).await {
        Ok(_) => Ok(warp::reply::with_status(
            warp::reply::json(&common::runner::EmptyResponse {}),
            StatusCode::OK,
        )),
        Err(err) => Ok(warp::reply::with_status(
            warp::reply::json(&common::runner::ErrorResponse {
                message: err.to_string(),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

pub async fn reload_project(
    call_context: CallContext,
    project_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    call_context
        .check_authorized(Some(&project_id), ActionType::Write)
        .await?;

    match reload_project_impl(call_context, project_id).await {
        Ok(_) => Ok(warp::reply::with_status(
            warp::reply::json(&common::runner::EmptyResponse {}),
            StatusCode::OK,
        )),
        Err(err) => Ok(warp::reply::with_status(
            warp::reply::json(&common::runner::ErrorResponse {
                message: err.to_string(),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

async fn reload_config_impl(call_context: CallContext) -> Result<(), anyhow::Error> {
    reload_impl(call_context, ActionEvent::ConfigReloaded).await?;
    Ok(())
}

async fn reload_project_impl(
    call_context: CallContext,
    project_id: String,
) -> Result<(), anyhow::Error> {
    reload_impl(call_context, ActionEvent::ProjectReloaded { project_id }).await?;
    Ok(())
}

async fn reload_impl(call_context: CallContext, event: ActionEvent) -> Result<(), anyhow::Error> {
    call_context.store.context().reload_config().await?;
    trigger_projects_impl(call_context, event).await;
    Ok(())
}

pub async fn trigger_projects_impl(call_context: CallContext, trigger: super::config::ActionEvent) {
    tokio::spawn(async move {
        match trigger_projects_impl_result(call_context, trigger).await {
            Result::Err(err) => {
                error!("Failed to match actions: {}", err);
            }
            Result::Ok(_) => {}
        }
    });
}

pub async fn trigger_projects_impl_result(
    call_context: CallContext,
    event: super::config::ActionEvent,
) -> Result<(), anyhow::Error> {
    let mut matched = call_context.get_actions(event).await?;

    if matched.reload_config {
        if call_context
            .check_allowed::<&str>(None, super::config::ActionType::Write)
            .await
        {
            return Err(anyhow::anyhow!(
                "Reloading config is not allowed, do nothing"
            ));
        }
    }

    // FIXME: Do it separately?
    if matched.reload_config || !matched.reload_projects.is_empty() {
        call_context.store.context().reload_config().await?;
        matched.merge(
            call_context
                .get_actions(super::config::ActionEvent::ConfigReloaded)
                .await?,
        );
    }

    let mut new_matcheds = Vec::new();
    for project_id in matched.reload_projects.iter() {
        if !call_context
            .check_allowed(Some(&project_id), ActionType::Write)
            .await
        {
            warn!("Not allowed to reload project {}, do nothing", project_id);
            continue;
        }

        new_matcheds.push(
            call_context
                .get_actions(super::config::ActionEvent::ProjectReloaded {
                    project_id: project_id.clone(),
                })
                .await?,
        );
    }
    for new_matched in new_matcheds.into_iter() {
        matched.merge(new_matched);
    }

    let execution_context = call_context.to_execution_context().await;
    execution_context
        .config()
        .run_project_actions(&execution_context, matched)
        .await?;

    Ok(())
}
