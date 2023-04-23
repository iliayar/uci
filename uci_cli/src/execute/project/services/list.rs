use crate::execute;

use log::*;
use termion::{color, style};

pub async fn execute_services_list(
    config: &crate::config::Config,
    project_id: String,
) -> Result<(), execute::ExecuteError> {
    debug!("Executing services list command");

    let response = crate::runner::api::list_services(config, project_id).await?;

    println!("{}Services{}:", style::Bold, style::Reset);
    for service in response.services.into_iter() {
        let status_string = match service.status {
            common::runner::ServiceStatus::Running => "running",
            common::runner::ServiceStatus::Starting => "starting",
            common::runner::ServiceStatus::NotRunning => "not running",
            common::runner::ServiceStatus::Dead => "dead",
            common::runner::ServiceStatus::Exited => "exited",
            common::runner::ServiceStatus::Restarting => "restartin",
            common::runner::ServiceStatus::Unknown => "unknown",
        };
        let color = match service.status {
            common::runner::ServiceStatus::Running => color::Green.fg_str(),
            common::runner::ServiceStatus::Starting => color::Blue.fg_str(),
            common::runner::ServiceStatus::NotRunning => color::LightBlack.fg_str(),
            common::runner::ServiceStatus::Dead => color::Red.fg_str(),
            common::runner::ServiceStatus::Exited => color::LightBlack.fg_str(), // FIXME: Make depent on exit code
            common::runner::ServiceStatus::Restarting => color::Blue.fg_str(),
            common::runner::ServiceStatus::Unknown => color::LightBlack.fg_str(),
        };
        println!(
            "- [{}] {}{}{}",
            status_string,
            color,
            service.id,
            style::Reset
        );
    }

    Ok(())
}
