use crate::{cli::*, execute};

pub async fn execute_repo(
    config: &crate::config::Config,
    command: RepoCommand,
) -> Result<(), execute::ExecuteError> {
    match command {
        RepoCommand::Update {
            repo,
            source,
            dry_run,
            update_only,
        } => super::update::execute_repo_update(config, repo, source, dry_run, update_only).await?,
        RepoCommand::List {} => super::list::execute_repos_list(config).await?,
    }

    Ok(())
}
