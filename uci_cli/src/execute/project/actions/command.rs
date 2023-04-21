use crate::{cli::*, execute};

pub async fn execute_action(
    config: &crate::config::Config,
    command: ActionCommand,
) -> Result<(), execute::ExecuteError> {
    match command {
        ActionCommand::Call { project, action } => {
            super::call::execute_action_call(config, project, action).await?
        }
        ActionCommand::List { project } => {
            super::list::execute_action_list(config, project).await?
        }
    }

    Ok(())
}
