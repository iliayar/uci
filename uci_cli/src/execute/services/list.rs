use crate::execute;

use log::*;
use termion::{color, style};

pub async fn execute_services_list(
    config: &crate::config::Config,
) -> Result<(), execute::ExecuteError> {
    let project_id = config.get_project().await;
    debug!("Executing services list command");

    let response = crate::runner::api::list_services(config, project_id).await?;

    println!("{}Services{}:", style::Bold, style::Reset);
    for service in response.services.into_iter() {
        let status_string = match service.status {
            models::ServiceStatus::Running => "running".to_string(),
            models::ServiceStatus::Starting => "starting".to_string(),
            models::ServiceStatus::NotRunning => "not running".to_string(),
            models::ServiceStatus::Dead => "dead".to_string(),
            models::ServiceStatus::Exited(code) => format!("exited ({})", code),
            models::ServiceStatus::Restarting => "restartin".to_string(),
            models::ServiceStatus::Unknown => "unknown".to_string(),
        };
        let color = match service.status {
            models::ServiceStatus::Running => color::Green.fg_str(),
            models::ServiceStatus::Starting => color::Blue.fg_str(),
            models::ServiceStatus::NotRunning => color::LightBlack.fg_str(),
            models::ServiceStatus::Dead => color::Red.fg_str(),
            models::ServiceStatus::Exited(0) => color::LightBlack.fg_str(),
            models::ServiceStatus::Exited(_) => color::Red.fg_str(),
            models::ServiceStatus::Restarting => color::Blue.fg_str(),
            models::ServiceStatus::Unknown => color::LightBlack.fg_str(),
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
