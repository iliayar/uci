use std::collections::HashSet;

use crate::lib::{
    config::{ActionEvent, ActionType},
    filters::{with_call_context, CallContext, ContextStore},
};

use warp::Filter;

pub fn filter(
    context: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_call_context(context, worker_context))
        .and(warp::path!("projects" / "list"))
        .and(warp::get())
        .and_then(list_projects)
}

async fn list_projects(call_context: CallContext) -> Result<impl warp::Reply, warp::Rejection> {
    let execution_context = call_context.to_execution_context().await;

    let projects: HashSet<String> = execution_context
        .config()
        .projects
        .list_projects()
        .into_iter()
        .filter(|project_id| execution_context.check_allowed(Some(project_id), ActionType::Read))
        .collect();

    let response = common::runner::ProjectsListResponse {
	projects
    };
    Ok(warp::reply::json(&response))
}
