use runner_lib::{call_context, config};

use crate::filters::{with_call_context, InternalServerError};

use log::*;
use reqwest::StatusCode;
use warp::Filter;

use anyhow::anyhow;

pub fn filter<PM: config::ProjectsManager + 'static>(
    deps: call_context::Deps<PM>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_call_context(deps))
        .and(warp::path!("projects" / "services" / "logs"))
        .and(warp::query::<common::runner::ServiceLogsQuery>())
        .and(warp::get())
        .and_then(service_logs)
}

async fn service_logs<PM: config::ProjectsManager + 'static>(
    call_context: call_context::CallContext<PM>,
    query: common::runner::ServiceLogsQuery,
) -> Result<impl warp::Reply, warp::Rejection> {
    match service_logs_impl(call_context, query).await {
        Ok(resp) => Ok(warp::reply::with_status(
            warp::reply::json(&resp),
            StatusCode::OK,
        )),
        Err(err) => Err(warp::reject::custom(InternalServerError::Error(
            err.to_string(),
        ))),
    }
}

async fn service_logs_impl<PM: config::ProjectsManager + 'static>(
    mut call_context: call_context::CallContext<PM>,
    common::runner::ServiceLogsQuery {
        project_id,
        service_id,
        follow,
        tail,
    }: common::runner::ServiceLogsQuery,
) -> Result<common::runner::ContinueReponse, anyhow::Error> {
    if !call_context
        .check_permissions(Some(&project_id), config::ActionType::Read)
        .await
    {
        return Err(anyhow!("No permissions for viewing logs in project"));
    }

    let run_id = call_context.init_run_buffered().await;
    tokio::spawn(async move {
        if let Err(err) = call_context
            .run_service_action(
                &project_id,
                &service_id,
                config::ServiceAction::Logs { follow, tail },
            )
            .await
        {
            error!("View logs failed: {}", err)
        }
        call_context.finish_run().await;
    });
    return Ok(common::runner::ContinueReponse { run_id });
}
