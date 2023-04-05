use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use crate::lib::git;

use super::LoadConfigError;

use anyhow::anyhow;
use log::*;

#[derive(Debug)]
pub struct Repos {
    repos: HashMap<String, Repo>,
}

#[derive(Debug)]
pub enum Repo {
    Regular { source: String, branch: String },
    Manual {},
}

pub type ReposDiffs = HashMap<String, git::ChangedFiles>;

impl Repo {
    async fn clone_if_missing(
        &self,
        id: &str,
        config: &super::ServiceConfig,
    ) -> Result<(), super::ExecutionError> {
        let path = config.repos_path.join(id);
        match self {
            Repo::Regular { source, branch } => {
                if !git::check_exists(path.clone()).await? {
                    info!("Cloning repo {}", id);

                    git::clone(
                        // TODO: Support http
                        source.strip_prefix("ssh://").unwrap().to_string(),
                        path,
                    )
                    .await?;
                } else {
                    info!("Repo {} already cloned", id);
                }
            }
            Repo::Manual {} => {
                info!("Repo {} is manually managed, don't clone", id);
                tokio::fs::create_dir_all(path).await?;
            }
        }

        Ok(())
    }

    async fn pull(
        &self,
        id: &str,
        config: &super::ServiceConfig,
    ) -> Result<git::ChangedFiles, super::ExecutionError> {
        match self {
            Repo::Regular { source, branch } => {
                let repo_path = config.repos_path.join(id);

                Ok(git::pull(repo_path, branch.clone()).await?)
            }

            Repo::Manual {} => {
                info!("Repo {} is manually managed, don't pull", id);
                Ok(git::ChangedFiles::default())
            }
        }
    }
}

impl Repos {
    pub async fn load<'a>(context: &super::LoadContext<'a>) -> Result<Repos, LoadConfigError> {
        raw::load(context).await
    }

    pub async fn pull_all(
        &self,
        config: &super::ServiceConfig,
        repos: &[String],
    ) -> Result<ReposDiffs, super::ExecutionError> {
        info!("Pulling repos");

        let mut repo_diffs = ReposDiffs::new();
        for repo_id in repos.iter() {
            let repo = self
                .repos
                .get(repo_id)
                .ok_or(anyhow!("No such repo: {}", repo_id))?;

            repo_diffs.insert(repo_id.clone(), repo.pull(repo_id, config).await?);
        }
        debug!("Repos diffs: {:?}", repo_diffs);

        Ok(repo_diffs)
    }

    pub async fn clone_missing_repos(
        &self,
        config: &super::ServiceConfig,
    ) -> Result<(), super::ExecutionError> {
        let mut git_tasks = Vec::new();

        for (id, repo) in self.repos.iter() {
            git_tasks.push(repo.clone_if_missing(id, config));
        }

        futures::future::try_join_all(git_tasks).await?;

        Ok(())
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::lib::{config, utils};
    use anyhow::anyhow;

    const REPO_CONFIG: &str = "repos.yaml";

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Repo {
        source: Option<String>,
        branch: Option<String>,
        manual: Option<bool>,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Repos {
        repos: HashMap<String, Repo>,
    }

    impl config::LoadRawSync for Repos {
        type Output = super::Repos;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            Ok(super::Repos {
                repos: self.repos.load_raw(context)?,
            })
        }
    }

    impl config::LoadRawSync for Repo {
        type Output = super::Repo;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            if !self.manual.unwrap_or(false) {
                Ok(super::Repo::Regular {
                    source: self
                        .source
                        .ok_or(anyhow!("'source' must be specified for not manual repo"))?,
                    branch: self.branch.unwrap_or(String::from("master")),
                })
            } else {
                Ok(super::Repo::Manual {})
            }
        }
    }

    pub async fn load<'a>(
        context: &config::LoadContext<'a>,
    ) -> Result<super::Repos, super::LoadConfigError> {
        config::load_sync::<Repos>(context.configs_root()?.join(REPO_CONFIG), context).await
    }
}
