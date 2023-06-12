use crate::cli::*;

use crate::utils::WithSpinner;

use log::*;
use termion::{color, style};

use runner_client::*;

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

    let response: models::EmptyResponse = async {
        let response = post(config, "/reload")?.send().await;
        json(response).await
    }
    .with_spinner("Updating config")
    .await?;

    println!("{}Config reloaded{}", color::Fg(color::Green), style::Reset);

    Ok(())
}
