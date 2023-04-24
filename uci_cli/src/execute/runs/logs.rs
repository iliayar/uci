use crate::execute;

use log::*;
use termion::{color, style};

pub async fn execute_runs_logs(
    config: &crate::config::Config,
    project_id: String,
    pipeline_id: String,
    run_id: String,
    follow: bool,
    status: bool,
) -> Result<(), execute::ExecuteError> {
    debug!("Executing service logs command");

    let follow_ws_client = if follow {
        match crate::runner::ws(config, run_id.clone()).await {
            Ok(ws_client) => Some(ws_client),
            Err(err) => {
                println!(
                    "{}Will not follow run. Probably it is already finished{}",
                    color::Fg(color::Yellow),
                    style::Reset
                );
                None
            }
        }
    } else {
        None
    };

    let runs_list = if status {
        Some(
            crate::runner::api::runs_list(
                config,
                Some(project_id.clone()),
                Some(pipeline_id.clone()),
            )
            .await?,
        )
    } else {
        None
    };

    let query = common::runner::RunsLogsRequestQuery {
        run: run_id.clone(),
        project: project_id,
        pipeline: pipeline_id,
    };
    let response = crate::runner::get_query(config, "/runs/logs", &query)?
        .send()
        .await;
    let response: common::runner::ContinueReponse = crate::runner::json(response).await?;

    debug!("Will follow run {}", response.run_id);

    let mut ws_client = crate::runner::ws(config, response.run_id).await?;

    match (follow_ws_client, runs_list) {
        (None, None) => {
            execute::utils::print_pipeline_run_no_state(&mut ws_client).await?;
        }
        (None, Some(runs_list)) => {
            execute::utils::print_pipeline_run_init(&mut ws_client, run_id, runs_list).await?;
        }
        (Some(mut follow_ws_client), None) => {
            execute::utils::print_pipeline_run_no_state_follow(
                &mut ws_client,
                &mut follow_ws_client,
            )
            .await?;
        }
        (Some(mut follow_ws_client), Some(runs_list)) => {
            execute::utils::print_pipeline_run_follow_init(
                &mut ws_client,
                &mut follow_ws_client,
                run_id,
                runs_list,
            )
            .await?;
        }
    }

    Ok(())
}
