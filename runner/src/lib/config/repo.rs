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
    Regular {
        path: PathBuf,
        source: String,
        branch: String,
    },
    Manual {
        path: PathBuf,
    },
}

pub type ReposDiffs = HashMap<String, git::ChangedFiles>;

impl Repo {
    async fn clone_if_missing(
        &self,
        config: &super::ServiceConfig,
    ) -> Result<(), super::ExecutionError> {
        match self {
            Repo::Regular {
                source,
                branch,
                path,
            } => {
                if !git::check_exists(path.clone()).await? {
                    git::clone(
                        // TODO: Support http
                        source.strip_prefix("ssh://").unwrap().to_string(),
                        path.clone(),
                    )
                    .await?;
                } else {
                    info!("Repo already cloned");
                }
            }
            Repo::Manual { path } => {
                info!("Repo is manually managed, don't clone");
                tokio::fs::create_dir_all(path.clone()).await?;
            }
        }

        Ok(())
    }

    async fn pull(
        &self,
        config: &super::ServiceConfig,
    ) -> Result<git::ChangedFiles, super::ExecutionError> {
        match self {
            Repo::Regular {
                source,
                branch,
                path,
            } => Ok(git::pull(path.clone(), branch.clone()).await?),

            Repo::Manual { path } => {
                info!("Repo is manually managed, don't pull");
                Ok(git::ChangedFiles::default())
            }
        }
    }
}

impl Into<common::vars::Vars> for &Repo {
    fn into(self) -> common::vars::Vars {
        use common::vars::*;
        let path = match self {
            Repo::Regular {
                source,
                branch,
                path,
            } => path.clone(),
            Repo::Manual { path } => path.clone(),
        };
        let value = HashMap::from_iter([(
            String::from("path"),
            Value::<()>::String(path.to_string_lossy().to_string()),
        )]);
        value.into()
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
            info!("Pulling repo {}", repo_id);

            repo_diffs.insert(repo_id.clone(), repo.pull(config).await?);
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
            info!("Cloning repo {}", id);
            git_tasks.push(repo.clone_if_missing(config));
        }

        futures::future::try_join_all(git_tasks).await?;

        Ok(())
    }
}

impl Into<common::vars::Vars> for &Repos {
    fn into(self) -> common::vars::Vars {
        let mut value: HashMap<String, common::vars::Vars> = HashMap::new();
        for (id, repo) in self.repos.iter() {
            value.insert(id.to_string(), repo.into());
        }
        value.into()
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
        path: Option<String>,
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
            let default_path = context.config()?.repos_path.join(context.extra("_id")?);
            let path = self
                .path
                .map(|path| utils::try_expand_home(path))
                .unwrap_or(default_path);
            if !self.manual.unwrap_or(false) {
                Ok(super::Repo::Regular {
                    source: self
                        .source
                        .ok_or(anyhow!("'source' must be specified for not manual repo"))?,
                    branch: self.branch.unwrap_or(String::from("master")),
                    path,
                })
            } else {
                Ok(super::Repo::Manual { path })
            }
        }
    }

    pub async fn load<'a>(
        context: &config::LoadContext<'a>,
    ) -> Result<super::Repos, super::LoadConfigError> {
        config::load_sync::<Repos>(context.configs_root()?.join(REPO_CONFIG), context).await
    }
}
