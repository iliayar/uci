use crate::execute;

use log::*;
use termion::style;

pub async fn execute_services_list(
    config: &crate::config::Config,
    project_id: String,
) -> Result<(), execute::ExecuteError> {
    debug!("Executing services list command");

    let response = crate::runner::get(config, format!("/projects/{}/services/list", project_id))?
        .send()
        .await;
    let response: common::runner::ServicesListResponse = crate::runner::json(response).await?;

    println!("{}Services{}:", style::Bold, style::Reset);
    for service in response.services.into_iter() {
        println!("- {}", service.id);
    }

    Ok(())
}
