use crate::imp::{
    config::{self, ActionType},
    filters::{with_call_context, AuthRejection, InternalServerError, Deps},
};

use reqwest::StatusCode;
use warp::Filter;

pub fn filter<PM: config::ProjectsManager>(
    deps: Deps<PM>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_call_context(deps))
        .and(warp::path!("reload"))
        .and(warp::post())
        .and_then(reload_config)
}

async fn reload_config<PM: config::ProjectsManager>(
    call_context: super::CallContext<PM>,
) -> Result<impl warp::Reply, warp::Rejection> {
    if !call_context
        .check_permissions(None, ActionType::Write)
        .await
    {
        return Err(warp::reject::custom(AuthRejection::TokenIsUnauthorized));
    }

    match call_context.reload_config().await {
        Ok(_) => Ok(warp::reply::with_status(
            warp::reply::json(&common::runner::EmptyResponse {}),
            StatusCode::OK,
        )),
        Err(err) => Err(warp::reject::custom(InternalServerError::Error(
            err.to_string(),
        ))),
    }
}
