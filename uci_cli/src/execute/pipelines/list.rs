use crate::execute;

use log::*;
use termion::style;

use runner_client::*;

pub async fn execute_pipelines_list(
    config: &crate::config::Config,
) -> Result<(), execute::ExecuteError> {
    let project_id = config.get_project().await;
    debug!("Executing pipelines list command");

    let query = models::ListPipelinesQuery { project_id };
    let response = get_query(config, "/projects/pipelines/list", &query)?
        .send()
        .await;
    let response: models::PipelinesListResponse = json(response).await?;

    println!("{}Pipelines{}:", style::Bold, style::Reset);
    for pipeline in response.pipelines.into_iter() {
        println!("- {}", pipeline.id);
    }

    Ok(())
}
