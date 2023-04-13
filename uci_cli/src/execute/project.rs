use crate::cli::*;

use log::*;
use termion::style;

pub async fn execute_project(
    config: &crate::config::Config,
    command: ProjectCommands,
) -> Result<(), super::ExecuteError> {
    match command {
        ProjectCommands::List {} => execute_project_list(config).await?,
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
