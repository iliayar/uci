use crate::{cli::*, execute};

pub async fn execute_project(
    config: &crate::config::Config,
    command: ProjectCommands,
) -> Result<(), execute::ExecuteError> {
    match command {
        ProjectCommands::List {} => super::list::execute_project_list(config).await?,
    }

    Ok(())
}
