use runner_lib::{call_context, config};

use crate::filters::{with_call_context, AuthRejection};

use reqwest::StatusCode;
use warp::Filter;

use log::*;

pub fn filter<PM: config::ProjectsManager + 'static>(
    deps: call_context::Deps<PM>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_call_context(deps))
        .and(warp::path!("call" / String / String))
        .and(warp::post())
        .and_then(call)
}

async fn call<PM: config::ProjectsManager + 'static>(
    mut call_context: call_context::CallContext<PM>,
    project_id: String,
    trigger_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    if !call_context
        .check_permissions(Some(&project_id), config::ActionType::Execute)
        .await
    {
        return Err(warp::reject::custom(AuthRejection::TokenIsUnauthorized));
    }

    let run_id = call_context.init_run_buffered().await;
    tokio::spawn(async move {
        if let Err(err) = call_context.call_trigger(&project_id, &trigger_id).await {
            error!("Call action failed: {}", err);
        }
        call_context.finish_run().await;
    });

    Ok(warp::reply::with_status(
        warp::reply::json(&common::runner::ContinueReponse { run_id }),
        StatusCode::ACCEPTED,
    ))
}
