use crate::{cli::*, execute};

pub async fn execute_run(
    config: &crate::config::Config,
    command: RunCommands,
) -> Result<(), execute::ExecuteError> {
    match command {
        RunCommands::List { pipeline } => super::list::execute_runs_list(config, pipeline).await?,
        RunCommands::Logs {
            pipeline,
            run_id,
            follow,
            status,
        } => super::logs::execute_runs_logs(config, pipeline, run_id, follow, status).await?,
        RunCommands::Cancel { pipeline, run_id } => {
            super::cancel::execute_runs_cancel(config, pipeline, run_id).await?
        }
    }

    Ok(())
}
