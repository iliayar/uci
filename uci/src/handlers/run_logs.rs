use common::{run_context::RunContext, state::State};
use futures::{pin_mut, StreamExt};
use runner_lib::{call_context, config};

use crate::filters::{with_call_context, InternalServerError};

use anyhow::anyhow;
use log::*;

use reqwest::StatusCode;
use warp::Filter;

pub fn filter(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_call_context(deps))
        .and(warp::path!("runs" / "logs"))
        .and(warp::query::<common::runner::RunsLogsRequestQuery>())
        .and(warp::get())
        .and_then(run_logs)
}

async fn run_logs(
    call_context: call_context::CallContext,
    query_params: common::runner::RunsLogsRequestQuery,
) -> Result<impl warp::Reply, warp::Rejection> {
    match run_logs_impl(call_context, query_params).await {
        Ok(resp) => Ok(warp::reply::with_status(
            warp::reply::json(&resp),
            StatusCode::OK,
        )),
        Err(err) => Err(warp::reject::custom(InternalServerError::Error(
            err.to_string(),
        ))),
    }
}

async fn run_logs_impl(
    mut call_context: call_context::CallContext,
    query: common::runner::RunsLogsRequestQuery,
) -> Result<common::runner::ContinueReponse, anyhow::Error> {
    if !call_context
        .check_permissions(Some(&query.project), config::ActionType::Read)
        .await
    {
        return Err(anyhow!("No permissions for viewing logs in project"));
    }

    let run_id = call_context.init_run_buffered().await;
    tokio::spawn(async move {
        if let Err(err) = call_context
            .with_state(|state| async move { run_logs_job(&state, query).await })
            .await
        {
            error!("Failed to view run logs: {}", err);
        }
        call_context.finish_run().await;
    });

    Ok(common::runner::ContinueReponse { run_id })
}

async fn run_logs_job<'a>(
    state: &State<'a>,
    common::runner::RunsLogsRequestQuery {
        run,
        project,
        pipeline,
    }: common::runner::RunsLogsRequestQuery,
) -> Result<(), anyhow::Error> {
    let executor: &worker_lib::executor::Executor = state.get()?;
    let run_context: &RunContext = state.get()?;

    if !run_context
        .wait_for_client(std::time::Duration::from_secs(5))
        .await
    {
        warn!("Will not wait for client more, aborting");
        return Ok(());
    }

    let logs = {
        let runs = executor.runs.lock().await;
        runs.logs(project, pipeline, run).await?
    };
    pin_mut!(logs);

    while let Some(log) = logs.next().await {
        let log = log?;
        run_context.send(log).await;
    }

    Ok(())
}
