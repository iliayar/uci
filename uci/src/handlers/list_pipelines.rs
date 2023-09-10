use runner_lib::{call_context, config};

use crate::filters::{with_call_context, InternalServerError};

use reqwest::StatusCode;
use warp::Filter;

use anyhow::anyhow;

pub fn filter(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(warp::path!("projects" / "pipelines" / "list"))
        .and(with_call_context(deps))
        .and(warp::query::<models::ListPipelinesQuery>())
        .and(warp::get())
        .and_then(list_pipelines)
}

async fn list_pipelines(
    call_context: call_context::CallContext,
    models::ListPipelinesQuery { project_id }: models::ListPipelinesQuery,
) -> Result<impl warp::Reply, warp::Rejection> {
    match list_pipelines_impl(call_context, &project_id).await {
        Ok(resp) => Ok(warp::reply::with_status(
            warp::reply::json(&resp),
            StatusCode::OK,
        )),
        Err(err) => Err(warp::reject::custom(InternalServerError::Error(
            err.to_string(),
        ))),
    }
}

async fn list_pipelines_impl(
    call_context: call_context::CallContext,
    project_id: &str,
) -> Result<models::PipelinesListResponse, anyhow::Error> {
    if !call_context
        .check_permissions(Some(project_id), config::permissions::ActionType::Read)
        .await
    {
        return Err(anyhow!("No permissions for viewing project"));
    }

    let project = call_context.get_project(project_id).await?;

    let mut pipelines = Vec::new();
    let pipelines_description = project.pipelines.list_pipelines().await;
    for pipeline in pipelines_description.pipelines.into_iter() {
        pipelines.push(models::Pipeline { id: pipeline.name });
    }

    Ok(models::PipelinesListResponse { pipelines })
}
