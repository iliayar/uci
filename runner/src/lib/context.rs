use super::{config::{Config, ConfigError}, utils::expand_home, git};

use log::*;
use thiserror::Error;

pub struct Context {
    config: Config,
}

#[derive(Debug, Error)]
pub enum ContextError {
    #[error("{0}")]
    ConfigError(#[from] ConfigError),

    #[error("Git error: {0}")]
    GitError(#[from] git::GitError),
}

impl Context {
    pub fn new(config: Config) -> Context {
        Context { config: config }
    }

    pub async fn clone_missing_repos(&self) -> Result<(), ContextError> {
        let mut git_tasks = Vec::new();

        for (id, repo) in self.config.repos.iter() {
            let path = expand_home("~/.microci/repos").join(id);

            if !git::check_exists(path.clone()).await? {
                info!("Cloning repo {}", id);
                let task = git::clone_ssh(
                    // TODO: Support http
                    repo.source.strip_prefix("ssh://").unwrap().to_string(),
                    path,
                );

                git_tasks.push(task);
            } else {
                info!("Repo {} already cloned", id);
            }

            // let task = git::pull_ssh(
            //     path,
            //     repo.branch.clone(),
            // );

            // info!("Commiting in repo {}", id);
            // let task = git::commit_all(
            //     expand_home("~/.microci/repos").join(id),
            //     String::from("Msg from microci"),
            // );

            // info!("Push in repo {}", id);
            // let task = git::push_ssh(
            //     expand_home("~/.microci/repos").join(id),
            //     String::from("main"),
            // );
        }

        for task in git_tasks.into_iter() {
            task.await?;
        }

        Ok(())
    }
}
