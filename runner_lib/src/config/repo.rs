use std::{collections::HashMap, path::PathBuf};

use crate::git;
use common::run_context::RunContext;

use anyhow::anyhow;
use common::state::State;
use log::*;

#[derive(Debug, Clone, Default)]
pub struct Repos {
    pub repos: HashMap<String, Repo>,
}

#[derive(Debug, Clone)]
pub enum Repo {
    Regular {
        id: String,
        path: PathBuf,
        source: String,
        branch: String,
    },
    Manual {
        id: String,
        path: PathBuf,
    },
}

pub enum Diff {
    Changes(git::ChangedFiles),
    Whole,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        match self {
            Diff::Changes(changes) => changes.is_empty(),
            Diff::Whole => false,
        }
    }
}

impl Repo {
    async fn clone_if_missing<'a>(&self, state: &State<'a>) -> Result<(), anyhow::Error> {
        match self {
            Repo::Regular {
                id,
                source,
                branch,
                path,
            } => {
                if !git::check_exists(path.clone()).await? {
                    let run_context: &RunContext = state.get()?;
                    run_context
                        .send(common::runner::CloneMissingRepos::ClonningRepo {
                            repo_id: id.to_string(),
                        })
                        .await;
                    git::clone(
                        // TODO: Support http
                        source.strip_prefix("ssh://").unwrap().to_string(),
                        path.clone(),
                    )
                    .await?;
                    run_context
                        .send(common::runner::CloneMissingRepos::RepoCloned {
                            repo_id: id.to_string(),
                        })
                        .await;
                } else {
                    info!("Repo {} already cloned", id);
                }
            }
            Repo::Manual { id, path } => {
                info!("Repo {} is manually managed, don't clone", id);
                tokio::fs::create_dir_all(path.clone()).await?;
            }
        }

        Ok(())
    }

    async fn update<'a>(
        &self,
        state: &State<'a>,
        artifact: Option<PathBuf>,
    ) -> Result<Diff, anyhow::Error> {
        let executor: &worker_lib::executor::Executor = state.get()?;
        let project_info: &super::ProjectInfo = state.get()?;

        match self {
            Repo::Regular {
                id,
                source,
                branch,
                path,
            } => {
                let _guard = executor.write_repo(&project_info.id, &id).await;

                if artifact.is_some() {
                    return Err(anyhow!(
                        "Artifact is specified for repo {}, but it's not manually managed. Wont update",
                        id
                    ));
                }

                if !git::check_exists(path.clone()).await? {
                    info!("Repo {} doesn't exists, will clone it", id);
                    self.clone_if_missing(state).await?;
                    Ok(Diff::Whole)
                } else {
                    Ok(Diff::Changes(
                        git::pull(path.clone(), branch.clone()).await?,
                    ))
                }
            }

            Repo::Manual { id, path } => {
                let _guard = executor.write_repo(&project_info.id, &id).await;

                let artifact = artifact.ok_or_else(|| {
                    anyhow!(
                        "Repo {} is manually managed, must provide source artifact",
                        id
                    )
                })?;

                let file = tokio::fs::File::open(artifact).await?;
                let mut archive = tokio_tar::Archive::new(file);

                // NOTE: haha, remove it all
                tokio::fs::remove_dir_all(path).await.ok();
                tokio::fs::create_dir_all(path).await?;

                archive.unpack(path).await?;
                Ok(Diff::Whole)
            }
        }
    }
}

impl From<&Repo> for common::vars::Vars {
    fn from(val: &Repo) -> Self {
        use common::vars::*;
        let path = match val {
            Repo::Regular {
                id,
                source,
                branch,
                path,
            } => path.clone(),
            Repo::Manual { id, path } => path.clone(),
        };
        let value = HashMap::from_iter([(
            String::from("path"),
            Value::<()>::String(path.to_string_lossy().to_string()),
        )]);
        value.into()
    }
}

impl Repos {
    pub async fn update_repo<'a>(
        &self,
        state: &State<'a>,
        repo_id: &str,
        artifact: Option<PathBuf>,
    ) -> Result<Diff, anyhow::Error> {
        let run_context: &RunContext = state.get()?;
        run_context
            .send(common::runner::UpdateRepoMessage::PullingRepo)
            .await;
        if let Some(repo) = self.repos.get(repo_id) {
            let res = repo.update(state, artifact).await;

            match res.as_ref() {
                Err(err) => {
                    run_context
                        .send(common::runner::UpdateRepoMessage::FailedToPull {
                            err: err.to_string(),
                        })
                        .await;
                }
                Ok(changed_files) => match changed_files {
                    Diff::Changes(changed_files) => {
                        run_context
                            .send(common::runner::UpdateRepoMessage::RepoPulled {
                                changed_files: changed_files.clone(),
                            })
                            .await;
                    }
                    Diff::Whole => {
                        run_context
                            .send(common::runner::UpdateRepoMessage::WholeRepoUpdated)
                            .await;
                    }
                },
            }

            res
        } else {
            run_context
                .send(common::runner::UpdateRepoMessage::NoSuchRepo)
                .await;
            Err(anyhow!("No such repo: {}", repo_id))
        }
    }

    pub async fn clone_missing_repos<'a>(&self, state: &State<'a>) -> Result<(), anyhow::Error> {
        let run_context: &RunContext = state.get()?;
        let mut git_tasks = Vec::new();

        run_context
            .send(common::runner::CloneMissingRepos::Begin)
            .await;

        for (id, repo) in self.repos.iter() {
            info!("Cloning repo {}", id);
            git_tasks.push(repo.clone_if_missing(state));
        }

        futures::future::try_join_all(git_tasks).await?;

        run_context
            .send(common::runner::CloneMissingRepos::Finish)
            .await;

        Ok(())
    }

    pub fn list_repos(&self) -> Vec<String> {
        self.repos.iter().map(|(k, _)| k.clone()).collect()
    }
}

impl From<&Repos> for common::vars::Vars {
    fn from(value: &Repos) -> Self {
        let mut vars: HashMap<String, common::vars::Vars> = HashMap::new();
        for (id, repo) in value.repos.iter() {
            vars.insert(id.to_string(), repo.into());
        }
        vars.into()
    }
}

pub mod repos_raw {
    pub use super::raw::Repo;
}

mod raw {
    use std::collections::HashMap;

    use common::state::State;
    use serde::{Deserialize, Serialize};

    use crate::{config, utils};
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

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            Ok(super::Repos {
                repos: self.repos.load_raw(state)?,
            })
        }
    }

    impl config::LoadRawSync for Repo {
        type Output = super::Repo;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            let repo_id: String = state.get_named("_id").cloned()?;
            let project_info: &config::ProjectInfo = state.get()?;
            let service_config: &config::ServiceConfig = state.get()?;
            let default_path = service_config
                .repos_path
                .join(format!("{}_{}", project_info.id, repo_id));
            let path = if let Some(path) = self.path {
                utils::eval_abs_path(state, path)?
            } else {
                default_path
            };
            if !self.manual.unwrap_or(false) {
                Ok(super::Repo::Regular {
                    id: repo_id,
                    source: self
                        .source
                        .ok_or_else(|| anyhow!("'source' must be specified for not manual repo"))?,
                    branch: self.branch.unwrap_or_else(|| String::from("master")),
                    path,
                })
            } else {
                Ok(super::Repo::Manual { id: repo_id, path })
            }
        }
    }
}
