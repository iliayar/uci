use crate::{cli::*, execute};

pub async fn execute_run(
    config: &crate::config::Config,
    command: RunCommands,
) -> Result<(), execute::ExecuteError> {
    match command {
        RunCommands::List { project, pipeline } => {
            super::list::execute_runs_list(config, project, pipeline).await?
        }
    }

    Ok(())
}
