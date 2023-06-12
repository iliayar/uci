use crate::execute;

use termion::style;

use log::*;

use runner_client::*;

pub async fn execute_repos_list(
    config: &crate::config::Config,
) -> Result<(), execute::ExecuteError> {
    debug!("Executing repos list command");
    let project = config.get_project().await;

    let response = api::repos_list(config, project).await?;

    println!("{}Repos{}:", style::Bold, style::Reset);
    for repo in response.repos.into_iter() {
        println!("- {}", repo.id);
    }

    Ok(())
}
