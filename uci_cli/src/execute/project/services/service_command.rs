use crate::execute;

use log::*;

pub async fn execute_services_command(
    config: &crate::config::Config,
    project_id: String,
    service: Option<Vec<String>>,
    command: common::runner::ServiceCommand,
) -> Result<(), execute::ExecuteError> {
    debug!("Executing service logs command");

    let services = if let Some(services) = service {
        services
    } else {
        crate::runner::api::list_services(config, project_id.clone())
            .await?
            .services
            .into_iter()
            .map(|s| s.id)
            .collect()
    };

    let body = common::runner::ServiceCommandRequest {
        project_id,
        services,
        command,
    };

    let response = crate::runner::post_body(config, "/projects/services/command", &body)?
        .send()
        .await;
    let response: common::runner::ContinueReponse = crate::runner::json(response).await?;

    debug!("Will follow run {}", response.run_id);

    let mut ws_client = crate::runner::ws(config, response.run_id).await?;

    execute::utils::print_clone_repos(&mut ws_client).await?;

    execute::utils::print_pipeline_run(&mut ws_client).await?;

    Ok(())
}
