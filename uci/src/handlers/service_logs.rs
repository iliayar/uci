use runner_lib::{call_context, config};

use crate::filters::{with_call_context, InternalServerError};

use log::*;
use reqwest::StatusCode;
use warp::Filter;

use anyhow::anyhow;

pub fn filter(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(warp::path!("projects" / "services" / "logs"))
        .and(with_call_context(deps))
        .and(warp::query::<models::ServiceLogsQuery>())
        .and(warp::body::json::<models::ServiceLogsBody>())
        .and(warp::get())
        .and_then(service_logs)
}

async fn service_logs(
    call_context: call_context::CallContext,
    query: models::ServiceLogsQuery,
    body: models::ServiceLogsBody,
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

async fn service_logs_impl(
    mut call_context: call_context::CallContext,
    models::ServiceLogsQuery { project_id }: models::ServiceLogsQuery,
    models::ServiceLogsBody {
        services,
        follow,
        tail,
    }: models::ServiceLogsBody,
) -> Result<models::ContinueReponse, anyhow::Error> {
    if !call_context
        .check_permissions(Some(&project_id), config::permissions::ActionType::Read)
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
                config::actions::ServiceAction::Logs { follow, tail },
            )
            .await
        {
            error!("View logs failed: {}", err)
        }
        call_context.finish_run().await;
    });

    Ok(models::ContinueReponse { run_id })
}
