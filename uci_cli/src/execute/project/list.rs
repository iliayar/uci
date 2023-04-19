use crate::execute;

use termion::style;

use log::*;

pub async fn execute_project_list(
    config: &crate::config::Config,
) -> Result<(), execute::ExecuteError> {
    debug!("Executing project list command");

    let response = crate::runner::get(config, "/projects/list")?.send().await;
    let response: common::runner::ProjectsListResponse = crate::runner::json(response).await?;

    println!("{}Projects{}:", style::Bold, style::Reset);
    for project in response.projects.into_iter() {
        println!("- {}", project.id);
    }

    Ok(())
}
