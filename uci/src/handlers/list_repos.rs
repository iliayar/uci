use runner_lib::{call_context, config};

use crate::filters::{with_call_context, InternalServerError};

use reqwest::StatusCode;
use warp::Filter;

use anyhow::anyhow;

pub fn filter<PM: config::ProjectsManager>(
    deps: call_context::Deps<PM>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_call_context(deps))
        .and(warp::path!("projects" / "repos" / "list"))
        .and(warp::query::<common::runner::ListReposQuery>())
        .and(warp::get())
        .and_then(list_repos)
}

async fn list_repos<PM: config::ProjectsManager>(
    call_context: call_context::CallContext<PM>,
    common::runner::ListReposQuery { project_id }: common::runner::ListReposQuery,
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

async fn list_repos_impl<PM: config::ProjectsManager>(
    call_context: call_context::CallContext<PM>,
    project_id: &str,
) -> Result<common::runner::ReposListResponse, anyhow::Error> {
    if !call_context
        .check_permissions(Some(project_id), config::ActionType::Read)
        .await
    {
        return Err(anyhow!("No permissions for viewing project"));
    }

    let project_info = call_context.get_project_info(project_id).await?;

    let mut repos = Vec::new();

    for repo_id in project_info.repos.list_repos().into_iter() {
	repos.push(common::runner::Repo {
	    id: repo_id,
	});
    }

    Ok(common::runner::ReposListResponse {
	repos
    })
}