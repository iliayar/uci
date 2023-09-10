use runner_lib::{call_context, config};

use crate::filters::{with_call_context, InternalServerError};

use reqwest::StatusCode;
use warp::Filter;

use anyhow::anyhow;

pub fn filter(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(warp::path!("projects" / "services" / "list"))
        .and(with_call_context(deps))
        .and(warp::query::<models::ListServicesQuery>())
        .and(warp::get())
        .and_then(list_services)
}

async fn list_services(
    call_context: call_context::CallContext,
    models::ListServicesQuery { project_id }: models::ListServicesQuery,
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

async fn list_services_impl(
    call_context: call_context::CallContext,
    project_id: &str,
) -> Result<models::ServicesListResponse, anyhow::Error> {
    if !call_context
        .check_permissions(Some(project_id), config::permissions::ActionType::Read)
        .await
    {
        return Err(anyhow!("No permissions for viewing project"));
    }

    let project = call_context.get_project(project_id).await?;

    let mut services = Vec::new();
    let services_description = project
        .services
        .list_services(call_context.state.as_ref())
        .await?;
    for service in services_description.services.into_iter() {
        let status = match service.status {
            worker_lib::docker::ContainerStatus::Running => models::ServiceStatus::Running,
            worker_lib::docker::ContainerStatus::NotRunning => models::ServiceStatus::NotRunning,
            worker_lib::docker::ContainerStatus::Starting => models::ServiceStatus::Starting,
            worker_lib::docker::ContainerStatus::Restarting => models::ServiceStatus::Restarting,
            worker_lib::docker::ContainerStatus::Dead => models::ServiceStatus::Dead,
            worker_lib::docker::ContainerStatus::Exited(code) => {
                models::ServiceStatus::Exited(code)
            }
            worker_lib::docker::ContainerStatus::Unknown => models::ServiceStatus::Unknown,
        };
        services.push(models::Service {
            id: service.name,
            status,
        });
    }

    Ok(models::ServicesListResponse { services })
}
