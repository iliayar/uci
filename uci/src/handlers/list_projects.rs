use runner_lib::{call_context, config};

use crate::filters::{with_call_context, InternalServerError};

use reqwest::StatusCode;
use warp::Filter;

pub fn filter(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(warp::path!("projects" / "list"))
        .and(with_call_context(deps))
        .and(warp::get())
        .and_then(list_projects)
}

async fn list_projects(
    call_context: call_context::CallContext,
) -> Result<impl warp::Reply, warp::Rejection> {
    match list_projects_impl(call_context).await {
        Ok(resp) => Ok(warp::reply::with_status(
            warp::reply::json(&resp),
            StatusCode::OK,
        )),
        Err(err) => Err(warp::reject::custom(InternalServerError::Error(
            err.to_string(),
        ))),
    }
}

async fn list_projects_impl(
    call_context: call_context::CallContext,
) -> Result<models::ProjectsListResponse, anyhow::Error> {
    let mut projects = Vec::new();
    for project in call_context.list_projects().await? {
        if call_context
            .check_permissions(Some(&project.id), config::permissions::ActionType::Read)
            .await
        {
            projects.push(models::Project { id: project.id });
        }
    }
    Ok(models::ProjectsListResponse { projects })
}
