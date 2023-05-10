use runner_lib::{call_context, config};

use crate::filters::{with_call_context, InternalServerError};

use reqwest::StatusCode;
use warp::Filter;

use anyhow::anyhow;

pub fn filter(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(warp::path!("projects" / "actions" / "list"))
        .and(with_call_context(deps))
        .and(warp::query::<common::runner::ListActionsQuery>())
        .and(warp::get())
        .and_then(list_actions)
}

async fn list_actions(
    call_context: call_context::CallContext,
    common::runner::ListActionsQuery { project_id }: common::runner::ListActionsQuery,
) -> Result<impl warp::Reply, warp::Rejection> {
    match list_actions_impl(call_context, &project_id).await {
        Ok(resp) => Ok(warp::reply::with_status(
            warp::reply::json(&resp),
            StatusCode::OK,
        )),
        Err(err) => Err(warp::reject::custom(InternalServerError::Error(
            err.to_string(),
        ))),
    }
}

async fn list_actions_impl(
    call_context: call_context::CallContext,
    project_id: &str,
) -> Result<common::runner::ActionsListResponse, anyhow::Error> {
    if !call_context
        .check_permissions(Some(project_id), config::ActionType::Read)
        .await
    {
        return Err(anyhow!("No permissions for viewing project"));
    }

    let project = call_context.get_project(project_id).await?;

    let mut actions = Vec::new();
    let actions_description = project.actions.list_actions().await;
    for action in actions_description.actions.into_iter() {
        actions.push(common::runner::Action { id: action.name });
    }

    Ok(common::runner::ActionsListResponse { actions })
}
