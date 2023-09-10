use runner_lib::{call_context, config};

use crate::filters::{with_call_context, InternalServerError};

use reqwest::StatusCode;
use warp::Filter;

use anyhow::anyhow;

pub fn filter(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(warp::path!("projects" / "repos" / "list"))
        .and(with_call_context(deps))
        .and(warp::query::<models::ListReposQuery>())
        .and(warp::get())
        .and_then(list_repos)
}

async fn list_repos(
    call_context: call_context::CallContext,
    models::ListReposQuery { project_id }: models::ListReposQuery,
) -> Result<impl warp::Reply, warp::Rejection> {
    match list_repos_impl(call_context, &project_id).await {
        Ok(resp) => Ok(warp::reply::with_status(
            warp::reply::json(&resp),
            StatusCode::OK,
        )),
        Err(err) => Err(warp::reject::custom(InternalServerError::Error(
            err.to_string(),
        ))),
    }
}

async fn list_repos_impl(
    call_context: call_context::CallContext,
    project_id: &str,
) -> Result<models::ReposListResponse, anyhow::Error> {
    if !call_context
        .check_permissions(Some(project_id), config::permissions::ActionType::Read)
        .await
    {
        return Err(anyhow!("No permissions for viewing project"));
    }

    let project_info = call_context.get_project_info(project_id).await?;

    let mut repos = Vec::new();

    for repo_id in project_info.repos.list_repos().into_iter() {
        repos.push(models::Repo { id: repo_id });
    }

    Ok(models::ReposListResponse { repos })
}
