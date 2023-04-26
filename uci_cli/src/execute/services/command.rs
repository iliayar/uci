use crate::{cli::*, execute};

pub async fn execute_service(
    config: &crate::config::Config,
    command: ServiceCommand,
) -> Result<(), execute::ExecuteError> {
    match command {
        ServiceCommand::List {} => super::list::execute_services_list(config).await?,
        ServiceCommand::Logs {
            service,
            follow,
            tail,
            all,
        } => super::logs::execute_services_logs(config, service, follow, tail, all).await?,
        ServiceCommand::Start {
            service,
            no_build,
            all,
        } => {
            super::service_command::execute_services_command(
                config,
                service,
                common::runner::ServiceCommand::Start { build: !no_build },
                all,
            )
            .await?
        }
        ServiceCommand::Stop { service, all } => {
            super::service_command::execute_services_command(
                config,
                service,
                common::runner::ServiceCommand::Stop,
                all,
            )
            .await?
        }
        ServiceCommand::Restart {
            service,
            no_build,
            all,
        } => {
            super::service_command::execute_services_command(
                config,
                service,
                common::runner::ServiceCommand::Restart { build: !no_build },
                all,
            )
            .await?
        }
    }

    Ok(())
}
