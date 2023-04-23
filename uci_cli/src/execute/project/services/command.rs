use crate::{cli::*, execute};

pub async fn execute_service(
    config: &crate::config::Config,
    command: ServiceCommand,
) -> Result<(), execute::ExecuteError> {
    match command {
        ServiceCommand::List { project } => {
            super::list::execute_services_list(config, project).await?
        }
        ServiceCommand::Logs {
            project,
            service,
            follow,
            tail,
        } => super::logs::execute_services_logs(config, project, service, follow, tail).await?,
        ServiceCommand::Start {
            project,
            service,
            no_build,
        } => {
            super::service_command::execute_services_command(
                config,
                project,
                service,
                common::runner::ServiceCommand::Start { build: !no_build },
            )
            .await?
        }
        ServiceCommand::Stop { project, service } => {
            super::service_command::execute_services_command(
                config,
                project,
                service,
                common::runner::ServiceCommand::Stop,
            )
            .await?
        }
        ServiceCommand::Restart {
            project,
            service,
            no_build,
        } => {
            super::service_command::execute_services_command(
                config,
                project,
                service,
                common::runner::ServiceCommand::Restart { build: !no_build },
            )
            .await?
        }
    }

    Ok(())
}
