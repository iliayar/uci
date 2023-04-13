use std::collections::HashSet;

use crate::lib::{
    config::{self, ActionEvent, ActionType},
    filters::{with_call_context, CallContext, ContextPtr, InternalServerError},
};

use reqwest::StatusCode;
use warp::Filter;

pub fn filter<PM: config::ProjectsManager>(
    context: ContextPtr<PM>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_call_context(context))
        .and(warp::path!("projects" / "list"))
        .and(warp::get())
        .and_then(list_projects)
}

async fn list_projects<PM: config::ProjectsManager>(
    call_context: CallContext<PM>,
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

async fn list_projects_impl<PM: config::ProjectsManager>(
    call_context: CallContext<PM>,
) -> Result<common::runner::ProjectsListResponse, anyhow::Error> {
    let projects: HashSet<String> = call_context
        .context
        .list_projects()
        .await?
        .into_iter()
        .filter_map(|project| {
            if project.check_allowed_token(call_context.token.as_ref(), ActionType::Read) {
                Some(project.id)
            } else {
                None
            }
        })
        .collect();

    Ok(common::runner::ProjectsListResponse { projects })
}
