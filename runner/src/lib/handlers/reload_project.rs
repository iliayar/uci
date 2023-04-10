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
        .and(warp::path!("reload" / String))
        .and(warp::post())
        .and_then(reload_project)
}

async fn reload_project(
    call_context: CallContext,
    project_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    call_context
        .check_authorized(Some(&project_id), ActionType::Write)
        .await?;

    info!("Reloading project {}", project_id);

    match super::reload_project_impl(call_context, project_id).await {
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
