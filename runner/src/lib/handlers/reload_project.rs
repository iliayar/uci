use crate::lib::{
    config::{self, ActionEvent, ActionType},
    filters::{with_call_context, CallContext, ContextPtr, InternalServerError, Unauthorized},
};

use reqwest::StatusCode;
use warp::Filter;

use log::*;

pub fn filter<PM: config::ProjectsManager>(
    context: ContextPtr<PM>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_call_context(context))
        .and(warp::path!("reload" / String))
        .and(warp::post())
        .and_then(reload_project)
}

async fn reload_project<PM: config::ProjectsManager>(
    call_context: CallContext<PM>,
    project_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    match call_context
        .context
        .get_project_info(project_id.clone())
        .await
    {
        Ok(project) => {
            if !project.check_allowed(call_context.token, ActionType::Write) {
                return Err(warp::reject::custom(Unauthorized::TokenIsUnauthorized));
            }
        }
        Err(err) => {
            return Err(warp::reject::custom(InternalServerError::Error(
                err.to_string(),
            )));
        }
    }

    // TODO: Trigger actions
    match call_context.context.reload_project(project_id).await {
        Ok(_) => Ok(warp::reply::with_status(
            warp::reply::json(&common::runner::EmptyResponse {}),
            StatusCode::OK,
        )),
        Err(err) => Err(warp::reject::custom(InternalServerError::Error(
            err.to_string(),
        ))),
    }
}
