use crate::cli::*;

use futures_util::StreamExt;
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
    let response: common::runner::ConfigReloadReponse = crate::runner::json(response).await?;

    if let Some(repos) = response.pulling_repos {
        println!(
            "{}Config still reloading{}",
            color::Fg(color::Blue),
            style::Reset
        );
        println!("Will pull {}repos{}:", style::Bold, style::Reset);
        for repo in repos.into_iter() {
            println!("- {}", repo);
        }
        debug!("ws client_id: {:?}", response.client_id);

        if let Some(client_id) = response.client_id {
            crate::runner::ws::<common::runner::ReloadConfigMessage>(config, client_id)
                .await
                .for_each(|msg| async move {
                    match msg {
                        common::runner::ReloadConfigMessage::ReposCloned => {
                            println!("{}Repos pulled{}", color::Fg(color::Green), style::Reset);
                        }
                        common::runner::ReloadConfigMessage::ConfigReloaded => {
                            println!("{}Config reloaded{}", color::Fg(color::Green), style::Reset);
                        }
                        common::runner::ReloadConfigMessage::ConfigReloadedError(err) => {
                            println!("{}{}{}", color::Fg(color::Red), err, style::Reset);
                        }
                    }
                })
                .await;
        }
    } else {
        println!("{}Config reloaded{}", color::Fg(color::Green), style::Reset);
    }

    Ok(())
}
