use crate::{cli::*, execute};

pub async fn execute_repo(
    config: &crate::config::Config,
    command: RepoCommand,
) -> Result<(), execute::ExecuteError> {
    match command {
        RepoCommand::Update { repo } => super::update::execute_repo_update(config, repo).await?,
    }

    Ok(())
}
