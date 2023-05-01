mod actions;
mod config;
mod pipelines;
mod project;
mod repos;
mod runs;
mod services;
mod upload;
mod utils;

use super::cli::*;

use log::*;

#[derive(Debug, thiserror::Error)]
pub enum ExecuteError {
    #[error("{0}")]
    Fatal(String),

    #[error("{0}")]
    Warning(String),

    #[error("Interrupted")]
    Interrupted,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ExecuteError {
    pub fn unexpected_message() -> ExecuteError {
        ExecuteError::Warning("Unexpected_message".to_string())
    }
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
        Commands::Actions { command } => actions::command::execute_action(config, command).await?,
        Commands::Repos { command } => repos::command::execute_repo(config, command).await?,
        Commands::Pipelines { command } => {
            pipelines::command::execute_pipeline(config, command).await?
        }
        Commands::Services { command } => {
            services::command::execute_service(config, command).await?
        }
        Commands::Upload { path } => upload::execute_upload(config, path).await?,
    }

    Ok(())
}
