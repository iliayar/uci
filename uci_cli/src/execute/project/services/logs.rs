use crate::execute;

use log::*;
use termion::{color, style};

pub async fn execute_services_logs(
    config: &crate::config::Config,
    project_id: String,
    service_id: String,
    follow: bool,
    tail: Option<usize>,
) -> Result<(), execute::ExecuteError> {
    debug!("Executing service logs command");

    let query = common::runner::ServiceLogsQuery {
        project_id,
        service_id,
        follow,

        // If follow then default tail to 10
        tail: if follow {
            Some(tail.unwrap_or(10))
        } else {
            tail
        },
    };
    let response = crate::runner::get_query(config, "/projects/services/logs", &query)?
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
                    color::Fg(color::Blue),
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
                }
            }
            _ => {}
        }
    }

    Ok(())
}
