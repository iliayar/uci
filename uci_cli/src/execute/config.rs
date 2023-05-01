use crate::cli::*;

use log::*;
use termion::{color, style};

pub async fn execute_config(
    config: &crate::config::Config,
    command: ConfigCommands,
) -> Result<(), super::ExecuteError> {
    #[allow(clippy::single_match)]
    #[allow(unreachable_patterns)]
    match command {
        ConfigCommands::Reload {} => execute_config_reload(config).await?,
        _ => {}
    }

    Ok(())
}

pub async fn execute_config_reload(
    config: &crate::config::Config,
) -> Result<(), super::ExecuteError> {
    debug!("Executing project list command");

    let response = crate::runner::post(config, "/reload")?.send().await;
    let response: common::runner::EmptyResponse = crate::runner::json(response).await?;

    println!("{}Config reloaded{}", color::Fg(color::Green), style::Reset);

    Ok(())
}
