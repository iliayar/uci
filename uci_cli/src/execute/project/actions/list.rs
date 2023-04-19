use crate::execute;

use log::*;
use termion::style;

pub async fn execute_action_list(
    config: &crate::config::Config,
    project_id: String,
) -> Result<(), execute::ExecuteError> {
    debug!("Executing action call command");

    let response = crate::runner::get(config, format!("/projects/{}/actions/list", project_id))?
        .send()
        .await;
    let response: common::runner::ActionsListResponse = crate::runner::json(response).await?;

    println!("{}Actions{}:", style::Bold, style::Reset);
    for action in response.actions.into_iter() {
        println!("- {}", action.id);
    }

    Ok(())
}