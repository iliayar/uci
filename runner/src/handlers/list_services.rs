use runner_lib::{call_context, config};

use crate::filters::{with_call_context, InternalServerError};

use reqwest::StatusCode;
use warp::Filter;

use anyhow::anyhow;

pub fn filter<PM: config::ProjectsManager>(
    deps: call_context::Deps<PM>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_call_context(deps))
        .and(warp::path!("projects" / String / "services" / "list"))
        .and(warp::get())
        .and_then(list_services)
}

async fn list_services<PM: config::ProjectsManager>(
    call_context: call_context::CallContext<PM>,
    project_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    match list_services_impl(call_context, &project_id).await {
        Ok(resp) => Ok(warp::reply::with_status(
            warp::reply::json(&resp),
            StatusCode::OK,
        )),
        Err(err) => Err(warp::reject::custom(InternalServerError::Error(
            err.to_string(),
        ))),
    }
}

async fn list_services_impl<PM: config::ProjectsManager>(
    call_context: call_context::CallContext<PM>,
    project_id: &str,
) -> Result<common::runner::ServicesListResponse, anyhow::Error> {
    if !call_context
        .check_permissions(Some(project_id), config::ActionType::Read)
        .await
    {
        return Err(anyhow!("No permissions for viewing project"));
    }

    let project = call_context.get_project(project_id).await?;

    let mut services = Vec::new();
    let services_description = project.services.list_services().await;
    for service in services_description.services.into_iter() {
        services.push(common::runner::Service { id: service.name });
    }

    Ok(common::runner::ServicesListResponse { services })
}
