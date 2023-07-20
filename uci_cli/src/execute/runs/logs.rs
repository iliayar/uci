use crate::execute;

use log::*;
use termion::{color, style};

use runner_client::*;

pub async fn execute_runs_logs(
    config: &crate::config::Config,
    pipeline_id: Option<String>,
    run_id: Option<String>,
    follow: bool,
    status: bool,
) -> Result<(), execute::ExecuteError> {
    let project_id = config.get_project().await;
    debug!("Executing service logs command");

    let (pipeline_id, run_id) = match (pipeline_id, run_id) {
        (Some(pipeline_id), Some(run_id)) => (pipeline_id, run_id),
        (pipeline_id, run_id) => {
            let run =
                crate::prompts::promp_run(config, Some(project_id.clone()), run_id, pipeline_id)
                    .await?;
            (run.pipeline_id, run.run_id)
        }
    };

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
        Some(api::list_runs(config, Some(project_id.clone()), Some(pipeline_id.clone())).await?)
    } else {
        None
    };

    let query = models::RunsLogsRequestQuery {
        run: run_id.clone(),
        project: project_id,
        pipeline: pipeline_id,
    };
    let response = api::run_logs(config, &query).await?;

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
