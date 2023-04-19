use crate::{cli::*, execute};

pub async fn execute_service(
    config: &crate::config::Config,
    command: ServiceCommand,
) -> Result<(), execute::ExecuteError> {
    match command {
        ServiceCommand::List { project_id } => {
            super::list::execute_services_list(config, project_id).await?
        }
    }

    Ok(())
}
