use crate::lib::{
    config::{ActionEvent, ActionType},
    filters::{with_call_context, CallContext, ContextStore},
};

use reqwest::StatusCode;
use warp::Filter;

use log::*;

pub fn filter(
    context: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_call_context(context, worker_context))
        .and(warp::path!("update" / String))
        .and(warp::post())
        .and_then(update_repo)
}

async fn update_repo(
    call_context: CallContext,
    repo: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    info!("Running repo {}", repo);
    super::trigger_projects_impl(call_context, ActionEvent::UpdateRepos { repos: vec![repo] })
        .await;
    Ok(StatusCode::OK)
}
