use crate::execute;

use log::*;
use termion::{color, style};

pub async fn execute_action_call(
    config: &crate::config::Config,
    project_id: String,
    action_id: String,
) -> Result<(), execute::ExecuteError> {
    debug!("Executing action call command");

    let response = crate::runner::post(config, format!("/call/{}/{}", project_id, action_id))?
        .send()
        .await;
    let response: common::runner::ContinueReponse = crate::runner::json(response).await?;

    debug!("Will follow run {}", response.run_id);
    let mut ws_client = crate::runner::ws(config, response.run_id).await?;

    println!(
        "{}Triggered action {}{}{} on project {}{}{} {}",
        color::Fg(color::Green),
        style::Bold,
        action_id,
        style::NoBold,
        style::Bold,
        project_id,
        style::NoBold,
        style::Reset
    );

    execute::utils::print_clone_repos(&mut ws_client).await?;

    Ok(())
}
