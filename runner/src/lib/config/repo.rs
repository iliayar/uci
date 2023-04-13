use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use crate::lib::git;

use anyhow::Error;

use anyhow::anyhow;
use log::*;

#[derive(Debug, Clone, Default)]
pub struct Repos {
    pub repos: HashMap<String, Repo>,
}

#[derive(Debug, Clone)]
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
    async fn clone_if_missing<'a>(
        &self,
        state: &super::State<'a>,
    ) -> Result<(), anyhow::Error> {
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

    async fn pull<'a>(
        &self,
        state: &super::State<'a>,
    ) -> Result<git::ChangedFiles, anyhow::Error> {
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
    pub async fn pull_repo<'a>(
        &self,
        state: &super::State<'a>,
        repo_id: &str,
    ) -> Result<git::ChangedFiles, anyhow::Error> {
	if let Some(repo) = self.repos.get(repo_id) {
	    repo.pull(state).await
	} else {
	    Err(anyhow!("No such repo: {}", repo_id))
	}
    }

    pub async fn clone_missing_repos<'a>(
        &self,
        state: &super::State<'a>,
    ) -> Result<(), anyhow::Error> {
        let mut git_tasks = Vec::new();

        for (id, repo) in self.repos.iter() {
            info!("Cloning repo {}", id);
            git_tasks.push(repo.clone_if_missing(state));
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

pub mod repos_raw {
    pub use super::raw::Repo;
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::lib::{config, utils};
    use anyhow::anyhow;

    const REPO_CONFIG: &str = "repos.yaml";

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct Repo {
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
            context: &config::State,
        ) -> Result<Self::Output, anyhow::Error> {
            Ok(super::Repos {
                repos: self.repos.load_raw(context)?,
            })
        }
    }

    impl config::LoadRawSync for Repo {
        type Output = super::Repo;

        fn load_raw(
            self,
            context: &config::State,
        ) -> Result<Self::Output, anyhow::Error> {
            let repo_id: String = context.get_named("_id").cloned()?;
            let project_info: &config::ProjectInfo = context.get()?;
            let service_config: &config::ServiceConfig = context.get()?;
            let default_path = service_config
                .repos_path
                .join(format!("{}_{}", project_info.id, repo_id));
            let path = if let Some(path) = self.path {
                utils::eval_abs_path(context, path)?
            } else {
                default_path
            };
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
}
