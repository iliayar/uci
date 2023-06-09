use crate::{cli::*, execute};

pub async fn execute_action(
    config: &crate::config::Config,
    command: ActionCommand,
) -> Result<(), execute::ExecuteError> {
    match command {
        ActionCommand::Call { action, dry_run } => super::call::execute_action_call(config, action, dry_run).await?,
        ActionCommand::List {} => super::list::execute_action_list(config).await?,
    }

    Ok(())
}
