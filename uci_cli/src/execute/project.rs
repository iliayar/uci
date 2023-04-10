use crate::cli::*;

use log::*;
use termion::{color, style};

pub async fn execute_project(
    config: &crate::config::Config,
    command: ProjectCommands,
) -> Result<(), super::ExecuteError> {
    match command {
        ProjectCommands::List {} => execute_project_list(config).await?,
        ProjectCommands::Reload { project } => execute_project_reload(config, project).await?,
    }

    Ok(())
}

pub async fn execute_project_list(
    config: &crate::config::Config,
) -> Result<(), super::ExecuteError> {
    debug!("Executing project list command");

    let response = crate::runner::get(config, "/projects/list")?.send().await;
    let response: common::runner::ProjectsListResponse = crate::runner::json(response).await?;

    println!("{}Projects{}:", style::Bold, style::Reset);
    for project_id in response.projects.into_iter() {
        println!("- {}", project_id);
    }

    Ok(())
}

pub async fn execute_project_reload(
    config: &crate::config::Config,
    project: String,
) -> Result<(), super::ExecuteError> {
    debug!("Executing project list command");

    let response = crate::runner::post(config, format!("/reload/{}", project))?
        .send()
        .await;
    let response: common::runner::EmptyResponse = crate::runner::json(response).await?;

    println!(
        "{}Project {}{}{} reloaded{}",
        color::Fg(color::Green),
        style::Bold,
        project,
        style::NoBold,
        style::Reset
    );

    Ok(())
}
