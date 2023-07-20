use crate::execute;

use log::*;
use termion::style;

use runner_client::*;

pub async fn execute_action_list(
    config: &crate::config::Config,
) -> Result<(), execute::ExecuteError> {
    let project_id = config.get_project().await;
    debug!("Executing action call command");

    let response = api::list_actions(config, project_id).await?;

    println!("{}Actions{}:", style::Bold, style::Reset);
    for action in response.actions.into_iter() {
        println!("- {}", action.id);
    }

    Ok(())
}
