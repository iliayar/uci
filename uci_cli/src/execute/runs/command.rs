use crate::{cli::*, execute};

pub async fn execute_run(
    config: &crate::config::Config,
    command: RunCommands,
) -> Result<(), execute::ExecuteError> {
    match command {
        RunCommands::List { project, pipeline } => {
            super::list::execute_runs_list(config, project, pipeline).await?
        }
        RunCommands::Logs {
            project,
            pipeline,
            run_id,
            follow,
            status,
        } => {
            super::logs::execute_runs_logs(config, project, pipeline, run_id, follow, status)
                .await?
        }
    }

    Ok(())
}
