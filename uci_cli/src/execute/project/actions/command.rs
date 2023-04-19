use crate::{cli::*, execute};

pub async fn execute_action(
    config: &crate::config::Config,
    command: ActionCommand,
) -> Result<(), execute::ExecuteError> {
    match command {
        ActionCommand::Call {
            project_id,
            action_id,
        } => super::call::execute_action_call(config, project_id, action_id).await?,
        ActionCommand::List { project_id } => {
            super::list::execute_action_list(config, project_id).await?
        }
    }

    Ok(())
}
