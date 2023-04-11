use crate::lib::{
    config::{self, ActionEvent, ActionType},
    filters::{with_call_context, CallContext, ContextPtr, InternalServerError, Unauthorized},
};

use reqwest::StatusCode;
use warp::Filter;

pub fn filter<PM: config::ProjectsManager>(
    context: ContextPtr<PM>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_call_context(context))
        .and(warp::path!("reload"))
        .and(warp::post())
        .and_then(reload_config)
}

async fn reload_config<PM: config::ProjectsManager>(
    call_context: CallContext<PM>,
) -> Result<impl warp::Reply, warp::Rejection> {
    if !call_context
        .context
        .config()
        .await
        .tokens
        .check_allowed(call_context.token, ActionType::Write)
    {
        return Err(warp::reject::custom(Unauthorized::TokenIsUnauthorized));
    }

    // TODO: Trigger actions
    match call_context.context.reload_config().await {
        Ok(_) => Ok(warp::reply::with_status(
            warp::reply::json(&common::runner::EmptyResponse {}),
            StatusCode::OK,
        )),
        Err(err) => Err(warp::reject::custom(InternalServerError::Error(
            err.to_string(),
        ))),
    }
}
