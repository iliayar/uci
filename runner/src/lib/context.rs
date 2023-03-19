use std::{borrow::BorrowMut, path::PathBuf, sync::Arc};

use super::{
    config::{self, Config, ConfigError},
    git,
    utils::expand_home,
};

use log::*;
use thiserror::Error;
use tokio::sync::Mutex;

pub struct Context {
    config_path: PathBuf,
    config: Arc<Mutex<Config>>,
}

#[derive(Debug, Error)]
pub enum ContextError {
    #[error("{0}")]
    ConfigError(#[from] ConfigError),

    #[error("Git error: {0}")]
    GitError(#[from] git::GitError),
}

impl Context {
    pub async fn new(config_path: PathBuf) -> Result<Context, ContextError> {
        let config = Config::load(config_path.clone()).await?;
        info!("Loaded config: {:#?}", config);

        Ok(Context {
            config_path,
            config: Arc::new(Mutex::new(config)),
        })
    }

    pub async fn clone_missing_repos(&self) -> Result<(), ContextError> {
        let mut git_tasks = Vec::new();
	let config = self.config.lock().await;

        for (id, repo) in config.repos.iter() {
	    let path = config.service_config.repos_path.join(id);

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

    pub async fn reload_config(&self) -> Result<(), ContextError> {
        let config = Config::load(self.config_path.clone()).await?;
        info!("Config reloaded {:#?}", config);

        *self.config.lock().await = config;

        Ok(())
    }
}
