use crate::execute;

use termion::style;

use log::*;

use runner_client::*;

pub async fn execute_project_list(
    config: &crate::config::Config,
) -> Result<(), execute::ExecuteError> {
    debug!("Executing project list command");

    let response = api::projects_list(config).await?;

    println!("{}Projects{}:", style::Bold, style::Reset);
    for project in response.projects.into_iter() {
        println!("- {}", project.id);
    }

    Ok(())
}
