use runner_lib::{call_context, config};

use crate::filters::{with_call_context, InternalServerError};

use anyhow::anyhow;

use reqwest::StatusCode;
use warp::Filter;

pub fn filter(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(warp::path!("runs" / "cancel"))
        .and(with_call_context(deps))
        .and(warp::body::json::<models::RunsCancelRequestBody>())
        .and(warp::post())
        .and_then(run_cancel)
}

async fn run_cancel(
    call_context: call_context::CallContext,
    body: models::RunsCancelRequestBody,
) -> Result<impl warp::Reply, warp::Rejection> {
    match run_cancel_impl(call_context, body).await {
        Ok(resp) => Ok(warp::reply::with_status(
            warp::reply::json(&resp),
            StatusCode::OK,
        )),
        Err(err) => Err(warp::reject::custom(InternalServerError::Error(
            err.to_string(),
        ))),
    }
}

async fn run_cancel_impl(
    call_context: call_context::CallContext,
    models::RunsCancelRequestBody {
        run,
        project,
        pipeline,
    }: models::RunsCancelRequestBody,
) -> Result<models::EmptyResponse, anyhow::Error> {
    if !call_context
        .check_permissions(Some(&project), config::permissions::ActionType::Execute)
        .await
    {
        return Err(anyhow!("No permissions for canceling run in project"));
    }

    let executor: &worker_lib::executor::Executor = call_context.state.get()?;
    let runs = executor.runs.lock().await;
    runs.cancel(project, pipeline, run).await?;

    Ok(models::EmptyResponse {})
}
