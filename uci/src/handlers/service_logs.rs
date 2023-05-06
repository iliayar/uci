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
        .and(warp::body::json::<common::runner::ServiceLogsBody>())
        .and(warp::get())
        .and_then(service_logs)
}

async fn service_logs<PM: config::ProjectsManager + 'static>(
    call_context: call_context::CallContext<PM>,
    query: common::runner::ServiceLogsQuery,
    body: common::runner::ServiceLogsBody,
) -> Result<impl warp::Reply, warp::Rejection> {
    match service_logs_impl(call_context, query, body).await {
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
    common::runner::ServiceLogsQuery { project_id }: common::runner::ServiceLogsQuery,
    common::runner::ServiceLogsBody {
        services,
        follow,
        tail,
    }: common::runner::ServiceLogsBody,
) -> Result<common::runner::ContinueReponse, anyhow::Error> {
    if !call_context
        .check_permissions(Some(&project_id), config::ActionType::Read)
        .await
    {
        return Err(anyhow!("No permissions for viewing logs in project"));
    }

    let run_id = call_context.init_run_buffered().await;
    tokio::spawn(async move {
        if !call_context
            .wait_for_clients(std::time::Duration::from_secs(2))
            .await
        {
            warn!("Will not wait for client more, aborting");
            return;
        }

        if let Err(err) = call_context
            .run_services_actions(
                &project_id,
                services,
                config::ServiceAction::Logs { follow, tail },
            )
            .await
        {
            error!("View logs failed: {}", err)
        }
        call_context.finish_run().await;
    });

    Ok(common::runner::ContinueReponse { run_id })
}
