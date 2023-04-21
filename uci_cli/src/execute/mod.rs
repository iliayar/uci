mod config;
mod project;
mod runs;
mod utils;

use super::cli::*;

use log::*;

#[derive(Debug, thiserror::Error)]
pub enum ExecuteError {
    #[error("{0}")]
    Fatal(String),

    #[error("{0}")]
    Warning(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub async fn execute(
    config: &super::config::Config,
    command: Commands,
) -> Result<(), ExecuteError> {
    match command {
        Commands::Projects { command } => {
            project::command::execute_project(config, command).await?
        }
        Commands::Runs { command } => runs::command::execute_run(config, command).await?,
        Commands::Config { command } => config::execute_config(config, command).await?,
    }

    Ok(())
}
