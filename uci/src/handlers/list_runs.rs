use std::collections::HashMap;

use runner_lib::{call_context, config};

use crate::filters::{with_call_context, InternalServerError};

use reqwest::StatusCode;
use warp::Filter;

pub fn filter(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_call_context(deps))
        .and(warp::path!("runs" / "list"))
        .and(warp::query::<common::runner::ListRunsRequestQuery>())
        .and(warp::get())
        .and_then(list_runs)
}

async fn list_runs(
    call_context: call_context::CallContext,
    query_params: common::runner::ListRunsRequestQuery,
) -> Result<impl warp::Reply, warp::Rejection> {
    match list_runs_impl(
        call_context,
        query_params.project_id,
        query_params.pipeline_id,
    )
    .await
    {
        Ok(resp) => Ok(warp::reply::with_status(
            warp::reply::json(&resp),
            StatusCode::OK,
        )),
        Err(err) => Err(warp::reject::custom(InternalServerError::Error(
            err.to_string(),
        ))),
    }
}

async fn list_runs_impl(
    call_context: call_context::CallContext,
    project_id: Option<String>,
    pipeline_id: Option<String>,
) -> Result<common::runner::ListRunsResponse, anyhow::Error> {
    let executor: &worker_lib::executor::Executor = call_context.state.get()?;
    let mut res = Vec::new();

    let projects = if let Some(project_id) = project_id {
        vec![project_id]
    } else {
        executor.runs.lock().await.get_projects()
    };

    for project in projects.into_iter() {
        if !call_context
            .check_permissions(Some(&project), config::ActionType::Read)
            .await
        {
            continue;
        }

        if let Some(project_runs) = executor.runs.lock().await.get_project_runs(&project) {
            let pipelines = if let Some(pipeline_id) = pipeline_id.as_ref() {
                vec![pipeline_id.clone()]
            } else {
                project_runs.get_pipelines()
            };

            for pipeline in pipelines.into_iter() {
                if let Some(pipeline_runs) = project_runs.get_pipeline_runs(&pipeline) {
                    for run in pipeline_runs.get_runs().into_iter() {
                        let status = match run.status().await {
                            worker_lib::executor::PipelineStatus::Starting => {
                                common::runner::RunStatus::Running
                            }
                            worker_lib::executor::PipelineStatus::Running => {
                                common::runner::RunStatus::Running
                            }
                            worker_lib::executor::PipelineStatus::Finished(finished_status) => {
                                common::runner::RunStatus::Finished(match finished_status {
                                    worker_lib::executor::PipelineFinishedStatus::Success => {
                                        common::runner::RunFinishedStatus::Success
                                    }
                                    worker_lib::executor::PipelineFinishedStatus::Error {
                                        message,
                                    } => common::runner::RunFinishedStatus::Error { message },
                                })
                            }
                        };

                        let mut jobs = HashMap::new();

                        for (id, job) in run.jobs().await.into_iter() {
                            let status = match job.status {
                                worker_lib::executor::JobStatus::Skipped => {
                                    common::runner::JobStatus::Skipped
                                }
                                worker_lib::executor::JobStatus::Pending => {
                                    common::runner::JobStatus::Pending
                                }
                                worker_lib::executor::JobStatus::Running { step } => {
                                    common::runner::JobStatus::Running { step }
                                }
                                worker_lib::executor::JobStatus::Finished { error } => {
                                    common::runner::JobStatus::Finished { error }
                                }
                            };
                            jobs.insert(id, common::runner::Job { status });
                        }

                        res.push(common::runner::Run {
                            project: project.clone(),
                            pipeline: pipeline.clone(),
                            run_id: run.id.clone(),
                            started: run.started,
                            stage: run.stage().await,
                            status,
                            jobs,
                        })
                    }
                }
            }
        }
    }

    Ok(common::runner::ListRunsResponse { runs: res })
}
