use crate::execute;

use log::*;
use termion::style;

pub async fn execute_action_list(
    config: &crate::config::Config,
) -> Result<(), execute::ExecuteError> {
    let project_id = config.get_project();
    debug!("Executing action call command");

    let response = crate::runner::api::actions_list(config, project_id).await?;

    println!("{}Actions{}:", style::Bold, style::Reset);
    for action in response.actions.into_iter() {
        println!("- {}", action.id);
    }

    Ok(())
}
