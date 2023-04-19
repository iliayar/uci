use crate::{cli::*, execute};

pub async fn execute_repo(
    config: &crate::config::Config,
    command: RepoCommand,
) -> Result<(), execute::ExecuteError> {
    match command {
        RepoCommand::Update {
            project_id,
            repo_id,
        } => super::update::execute_repo_update(config, project_id, repo_id).await?,
    }

    Ok(())
}
