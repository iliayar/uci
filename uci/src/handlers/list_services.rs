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
        .and(warp::query::<common::runner::ListServicesQuery>())
        .and(warp::get())
        .and_then(list_services)
}

async fn list_services(
    call_context: call_context::CallContext,
    common::runner::ListServicesQuery { project_id }: common::runner::ListServicesQuery,
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
) -> Result<common::runner::ServicesListResponse, anyhow::Error> {
    if !call_context
        .check_permissions(Some(project_id), config::ActionType::Read)
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
            worker_lib::docker::ContainerStatus::Running => common::runner::ServiceStatus::Running,
            worker_lib::docker::ContainerStatus::NotRunning => {
                common::runner::ServiceStatus::NotRunning
            }
            worker_lib::docker::ContainerStatus::Starting => {
                common::runner::ServiceStatus::Starting
            }
            worker_lib::docker::ContainerStatus::Restarting => {
                common::runner::ServiceStatus::Restarting
            }
            worker_lib::docker::ContainerStatus::Dead => common::runner::ServiceStatus::Dead,
            worker_lib::docker::ContainerStatus::Exited(code) => common::runner::ServiceStatus::Exited(code),
            worker_lib::docker::ContainerStatus::Unknown => common::runner::ServiceStatus::Unknown,
        };
        services.push(common::runner::Service {
            id: service.name,
            status,
        });
    }

    Ok(common::runner::ServicesListResponse { services })
}
