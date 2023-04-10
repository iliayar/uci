use crate::lib::{
    config::{ActionEvent, ActionType},
    filters::{with_call_context, CallContext, ContextStore},
};

use reqwest::StatusCode;
use warp::Filter;

pub fn filter(
    context: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_call_context(context, worker_context))
        .and(warp::path!("reload"))
        .and(warp::post())
        .and_then(reload_config)
}

async fn reload_config(call_context: CallContext) -> Result<impl warp::Reply, warp::Rejection> {
    call_context
        .check_authorized::<&str>(None, ActionType::Write)
        .await?;

    match super::reload_config_impl(call_context).await {
        Ok(reload_result) => {
            let response = common::runner::ConfigReloadReponse {
		client_id: reload_result.client_id,
                pulling_repos: reload_result.missing_repos,
            };

            if reload_result.done {
                Ok(warp::reply::with_status(
                    warp::reply::json(&response),
                    StatusCode::OK,
                ))
            } else {
                Ok(warp::reply::with_status(
                    warp::reply::json(&response),
                    StatusCode::ACCEPTED,
                ))
            }
        }
        Err(err) => Ok(warp::reply::with_status(
            warp::reply::json(&common::runner::ErrorResponse {
                message: err.to_string(),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}
