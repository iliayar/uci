use crate::execute;

use log::*;
use termion::{color, style};

use runner_client::*;

pub async fn execute_action_call(
    config: &crate::config::Config,
    action_id: Option<String>,
    dry_run: bool,
) -> Result<(), execute::ExecuteError> {
    let project_id = config.get_project().await;
    debug!("Executing action call command");

    let action_id = if let Some(action_id) = action_id {
        action_id
    } else {
        crate::prompts::promp_action(config, project_id.clone()).await?
    };

    let body = models::CallRequest {
        project_id,
        trigger_id: action_id,
        dry_run: Some(dry_run),
    };

    let response = api::action_call(config, &body).await?;

    debug!("Will follow run {}", response.run_id);
    let mut ws_client = crate::runner::ws(config, response.run_id).await?;

    println!(
        "{}Triggered action {}{}{} on project {}{}{} {}",
        color::Fg(color::Green),
        style::Bold,
        body.trigger_id,
        style::NoBold,
        style::Bold,
        body.project_id,
        style::NoBold,
        style::Reset
    );

    execute::utils::print_clone_repos(&mut ws_client).await?;

    execute::utils::print_pipeline_run(&mut ws_client).await?;

    Ok(())
}
