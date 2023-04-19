use crate::execute;

use log::*;
use termion::style;

pub async fn execute_pipelines_list(
    config: &crate::config::Config,
    project_id: String,
) -> Result<(), execute::ExecuteError> {
    debug!("Executing pipelines list command");

    let response = crate::runner::get(config, format!("/projects/{}/pipelines/list", project_id))?
        .send()
        .await;
    let response: common::runner::PipelinesListResponse = crate::runner::json(response).await?;

    println!("{}Pipelines{}:", style::Bold, style::Reset);
    for pipeline in response.pipelines.into_iter() {
        println!("- {}", pipeline.id);
    }

    Ok(())
}
