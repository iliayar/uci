use crate::{execute, utils::ucolor};

use log::*;
use termion::{color, style};

pub async fn execute_services_logs(
    config: &crate::config::Config,
    service: Option<Vec<String>>,
    follow: bool,
    tail: Option<usize>,
    all: bool,
) -> Result<(), execute::ExecuteError> {
    let project_id = config.get_project();
    debug!("Executing service logs command");

    let services = if let Some(services) = service {
        services
    } else if all {
        crate::runner::api::list_services(config, project_id.clone())
            .await?
            .services
            .into_iter()
            .map(|s| s.id)
            .collect()
    } else {
        crate::prompts::promp_services(config, project_id.clone()).await?
    };

    let body = common::runner::ServiceLogsBody {
        services,
        follow,

        // If follow then default tail to 10
        tail: if follow {
            Some(tail.unwrap_or(10))
        } else {
            tail
        },
    };

    let query = common::runner::ServiceLogsQuery { project_id };

    let response = crate::runner::get_query_body(config, "/projects/services/logs", &query, &body)?
        .send()
        .await;
    let response: common::runner::ContinueReponse = crate::runner::json(response).await?;

    debug!("Will follow run {}", response.run_id);

    let mut ws_client = crate::runner::ws(config, response.run_id).await?;

    execute::utils::print_clone_repos(&mut ws_client).await?;

    while let Some(message) = ws_client.receive::<common::runner::PipelineMessage>().await {
        match message {
            common::runner::PipelineMessage::ContainerLog {
                t,
                text,
                timestamp,
                container,
            } => {
                print!(
                    "{} [{}{}{}] ",
                    timestamp,
                    ucolor(&container),
                    container,
                    style::Reset
                );

                match t {
                    common::runner::LogType::Regular => println!("{}", text.trim_end()),
                    common::runner::LogType::Error => {
                        println!(
                            "{}{}{}",
                            color::Fg(color::Red),
                            text.trim_end(),
                            style::Reset
                        )
                    }
                    common::runner::LogType::Warning => {
                        println!(
                            "{}{}{}",
                            color::Fg(color::Yellow),
                            text.trim_end(),
                            style::Reset
                        )
                    }
                }
            }
            common::runner::PipelineMessage::Log {
                t: common::runner::LogType::Error,
                text,
                ..
            } => {
                println!(
                    "Failed to view logs: {}{}{}",
                    color::Fg(color::Red),
                    text.trim_end(),
                    style::Reset
                )
            }
            _ => {}
        }
    }

    Ok(())
}
