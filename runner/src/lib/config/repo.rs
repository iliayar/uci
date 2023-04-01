use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use crate::lib::git;

use super::LoadConfigError;

use anyhow::anyhow;
use log::*;

const REPO_CONFIG: &str = "repos.yaml";

#[derive(Debug)]
pub struct Repos {
    repos: HashMap<String, Repo>,
}

#[derive(Debug)]
pub struct Repo {
    source: String,
    branch: String,
}

pub type ReposDiffs = HashMap<String, git::ChangedFiles>;

impl Repos {
    pub async fn load(configs_root: PathBuf) -> Result<Repos, LoadConfigError> {
        raw::parse(configs_root.join(REPO_CONFIG)).await
    }

    pub async fn pull_all(
        &self,
        config: &super::ServiceConfig,
        repos: &[String],
    ) -> Result<ReposDiffs, super::ExecutionError> {
        info!("Pulling repos");

        let mut repo_diffs = ReposDiffs::new();
        for repo_id in repos.iter() {
            repo_diffs.insert(repo_id.clone(), self.pull(config, repo_id).await?);
        }
        debug!("Repos diffs: {:?}", repo_diffs);

        Ok(repo_diffs)
    }

    pub async fn pull(
        &self,
        config: &super::ServiceConfig,
        repo_id: &str,
    ) -> Result<git::ChangedFiles, super::ExecutionError> {
        let repo = self
            .repos
            .get(repo_id)
            .ok_or(anyhow!("No such repo: {}", repo_id))?;
        let repo_path = config.repos_path.join(repo_id);

        Ok(git::pull(repo_path, repo.branch.clone()).await?)
    }

    pub async fn clone_missing_repos(
        &self,
        config: &super::ServiceConfig,
    ) -> Result<(), super::ExecutionError> {
        let mut git_tasks = Vec::new();

        for (id, repo) in self.repos.iter() {
            let path = config.repos_path.join(id);

            if !git::check_exists(path.clone()).await? {
                info!("Cloning repo {}", id);

                git_tasks.push(git::clone(
                    // TODO: Support http
                    repo.source.strip_prefix("ssh://").unwrap().to_string(),
                    path,
                ));
            } else {
                info!("Repo {} already cloned", id);
            }
        }

        futures::future::try_join_all(git_tasks).await?;

        Ok(())
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::lib::config::utils;

    #[derive(Deserialize, Serialize)]
    struct Repo {
        source: String,
        branch: Option<String>,
    }

    #[derive(Deserialize, Serialize)]
    struct Repos {
        repos: HashMap<String, Repo>,
    }

    impl TryFrom<Repos> for super::Repos {
        type Error = super::LoadConfigError;

        fn try_from(value: Repos) -> Result<Self, Self::Error> {
            let mut repos = HashMap::new();

            for (id, Repo { source, branch }) in value.repos.into_iter() {
                repos.insert(
                    id,
                    super::Repo {
                        source,
                        branch: branch.unwrap_or(String::from("master")),
                    },
                );
            }

            Ok(super::Repos { repos })
        }
    }

    pub async fn parse(path: PathBuf) -> Result<super::Repos, super::LoadConfigError> {
        utils::load_file::<Repos, _>(path).await
    }
}
