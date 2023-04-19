use crate::{cli::*, execute};

pub async fn execute_project(
    config: &crate::config::Config,
    command: ProjectCommands,
) -> Result<(), execute::ExecuteError> {
    match command {
        ProjectCommands::List {} => super::list::execute_project_list(config).await?,
        ProjectCommands::Actions { command } => {
            super::actions::command::execute_action(config, command).await?
        }
        ProjectCommands::Repos { command } => {
            super::repos::command::execute_repo(config, command).await?
        }
        ProjectCommands::Pipelines { command } => {
            super::pipelines::command::execute_pipeline(config, command).await?
        }
        ProjectCommands::Services { command } => {
            super::services::command::execute_service(config, command).await?
        }
    }

    Ok(())
}
