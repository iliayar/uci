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
        .and(warp::path!("call" / String / String))
        .and(warp::post())
        .and_then(call)
}

async fn call(
    call_context: CallContext,
    project_id: String,
    trigger_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    info!("Running trigger {} for project {}", trigger_id, project_id);

    call_context
        .check_authorized(Some(&project_id), ActionType::Write)
        .await?;

    super::trigger_projects_impl(
        call_context,
        ActionEvent::DirectCall {
            project_id: project_id.clone(),
            trigger_id: trigger_id.clone(),
        },
    )
    .await;

    Ok(StatusCode::OK)
}
