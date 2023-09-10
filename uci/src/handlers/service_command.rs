use runner_lib::{call_context, config};

use crate::filters::{with_call_context, AuthRejection};

use reqwest::StatusCode;
use warp::Filter;

use log::*;

pub fn filter(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(warp::path!("projects" / "services" / "command"))
        .and(with_call_context(deps))
        .and(warp::body::json::<models::ServiceCommandRequest>())
        .and(warp::post())
        .and_then(service_command)
}

async fn service_command(
    mut call_context: call_context::CallContext,
    models::ServiceCommandRequest {
        project_id,
        services,
        command,
    }: models::ServiceCommandRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    if !call_context
        .check_permissions(Some(&project_id), config::permissions::ActionType::Execute)
        .await
    {
        return Err(warp::reject::custom(AuthRejection::TokenIsUnauthorized));
    }

    let service_action = match command {
        models::ServiceCommand::Stop => config::actions::ServiceAction::Stop,
        models::ServiceCommand::Start { build } => config::actions::ServiceAction::Start { build },
        models::ServiceCommand::Restart { build } => {
            config::actions::ServiceAction::Restart { build }
        }
    };

    let run_id = call_context.init_run_buffered().await;
    tokio::spawn(async move {
        if let Err(err) = call_context
            .run_services_actions(&project_id, services, service_action)
            .await
        {
            error!("Call action failed: {}", err);
        }
        call_context.finish_run().await;
    });

    Ok(warp::reply::with_status(
        warp::reply::json(&models::ContinueReponse { run_id }),
        StatusCode::ACCEPTED,
    ))
}
