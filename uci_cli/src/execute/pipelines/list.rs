use crate::execute;

use log::*;
use termion::style;

pub async fn execute_pipelines_list(
    config: &crate::config::Config,
) -> Result<(), execute::ExecuteError> {
    let project_id = config.get_project().await;
    debug!("Executing pipelines list command");

    let query = common::runner::ListPipelinesQuery { project_id };
    let response = crate::runner::get_query(config, "/projects/pipelines/list", &query)?
        .send()
        .await;
    let response: common::runner::PipelinesListResponse = crate::runner::json(response).await?;

    println!("{}Pipelines{}:", style::Bold, style::Reset);
    for pipeline in response.pipelines.into_iter() {
        println!("- {}", pipeline.id);
    }

    Ok(())
}
