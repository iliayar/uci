use std::{collections::HashMap, path::PathBuf};

use crate::git;
use common::run_context::RunContext;

use anyhow::{anyhow, Result};
use common::state::State;
use log::*;

use crate::config;

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
        commit: Option<String>,
    },
    Manual {
        id: String,
        path: PathBuf,
    },
}

pub enum Diff {
    Changes {
        changes: git::ChangedFiles,
        commit_message: String,
    },
    Whole,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        match self {
            Diff::Changes { changes, .. } => changes.is_empty(),
            Diff::Whole => false,
        }
    }
}

impl Repo {
    async fn clone_if_missing<'a>(&self, state: &State<'a>) -> Result<()> {
        match self {
            Repo::Regular {
                id,
                source,
                branch,
                path,
                ..
            } => {
                if !git::check_exists(path.clone()).await? {
                    let run_context: &RunContext = state.get()?;
                    run_context
                        .send(models::CloneMissingRepos::ClonningRepo {
                            repo_id: id.to_string(),
                        })
                        .await;
                    git::clone(source.clone(), path.clone()).await?;
                    run_context
                        .send(models::CloneMissingRepos::RepoCloned {
                            repo_id: id.to_string(),
                        })
                        .await;
                } else {
                    info!("Repo {} already cloned", id);
                }
            }
            Repo::Manual { id, path, .. } => {
                info!("Repo {} is manually managed, don't clone", id);
                tokio::fs::create_dir_all(path.clone()).await?;
            }
        }

        Ok(())
    }

    async fn update<'a>(&self, state: &State<'a>, artifact: Option<PathBuf>) -> Result<Diff> {
        let executor: &worker_lib::executor::Executor = state.get()?;
        let project_info: &config::projects::ProjectInfo = state.get()?;

        let dry_run = state
            .get::<worker_lib::executor::DryRun>()
            .cloned()
            .map(|v| v.0)
            .unwrap_or(false);

        match self {
            Repo::Regular {
                id,
                source,
                branch,
                path,
                ..
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
                    let pull_result = if dry_run {
                        git::fetch(path.clone(), branch.clone()).await?
                    } else {
                        git::pull(path.clone(), branch.clone()).await?
                    };
                    Ok(Diff::Changes {
                        changes: pull_result.changes,
                        commit_message: pull_result.commit_message,
                    })
                }
            }

            Repo::Manual { id, path, .. } => {
                let _guard = executor.write_repo(&project_info.id, &id).await;

                let artifact = artifact.ok_or_else(|| {
                    anyhow!(
                        "Repo {} is manually managed, must provide source artifact",
                        id
                    )
                })?;

                if !dry_run {
                    let file = tokio::fs::File::open(artifact).await?;
                    let mut archive = tokio_tar::Archive::new(file);

                    // NOTE: haha, remove it all
                    tokio::fs::remove_dir_all(path).await.ok();
                    tokio::fs::create_dir_all(path).await?;

                    archive.unpack(path).await?;
                }

                Ok(Diff::Whole)
            }
        }
    }
}

impl Repos {
    pub async fn update_repo<'a>(
        &self,
        state: &State<'a>,
        repo_id: &str,
        artifact: Option<PathBuf>,
    ) -> Result<Diff> {
        let run_context: &RunContext = state.get()?;
        run_context
            .send(models::UpdateRepoMessage::PullingRepo)
            .await;
        if let Some(repo) = self.repos.get(repo_id) {
            let res = repo.update(state, artifact).await;

            match res.as_ref() {
                Err(err) => {
                    run_context
                        .send(models::UpdateRepoMessage::FailedToPull {
                            err: err.to_string(),
                        })
                        .await;
                }
                Ok(changed_files) => match changed_files {
                    Diff::Changes {
                        changes,
                        commit_message,
                    } => {
                        run_context
                            .send(models::UpdateRepoMessage::RepoPulled {
                                changed_files: changes.clone(),
                            })
                            .await;
                    }
                    Diff::Whole => {
                        run_context
                            .send(models::UpdateRepoMessage::WholeRepoUpdated)
                            .await;
                    }
                },
            }

            res
        } else {
            run_context
                .send(models::UpdateRepoMessage::NoSuchRepo)
                .await;
            Err(anyhow!("No such repo: {}", repo_id))
        }
    }

    pub async fn clone_missing_repos<'a>(&self, state: &State<'a>) -> Result<()> {
        let run_context: &RunContext = state.get()?;
        let mut git_tasks = Vec::new();

        run_context.send(models::CloneMissingRepos::Begin).await;

        for (id, repo) in self.repos.iter() {
            info!("Cloning repo {}", id);
            git_tasks.push(repo.clone_if_missing(state));
        }

        futures::future::try_join_all(git_tasks).await?;

        run_context.send(models::CloneMissingRepos::Finish).await;

        Ok(())
    }

    pub fn list_repos(&self) -> Vec<String> {
        self.repos.iter().map(|(k, _)| k.clone()).collect()
    }
}

pub use dyn_obj::DynRepos;

mod dyn_obj {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize)]
    #[serde(transparent)]
    pub struct DynRepos {
        pub repos: HashMap<String, DynRepo>,
    }

    #[derive(Deserialize, Serialize)]
    pub struct DynRepo {
        pub path: PathBuf,
        pub branch: Option<String>,
        pub source: Option<String>,
        pub rev: Option<String>,
    }

    impl From<&super::Repo> for DynRepo {
        fn from(repo: &super::Repo) -> Self {
            match repo {
                super::Repo::Regular {
                    path,
                    source,
                    branch,
                    commit,
                    ..
                } => DynRepo {
                    path: path.clone(),
                    source: Some(source.clone()),
                    branch: Some(branch.clone()),
                    rev: commit.clone(),
                },
                super::Repo::Manual { path, .. } => DynRepo {
                    path: path.clone(),
                    branch: None,
                    source: None,
                    rev: None,
                },
            }
        }
    }

    impl From<&super::Repos> for DynRepos {
        fn from(repos: &super::Repos) -> Self {
            DynRepos {
                repos: repos
                    .repos
                    .iter()
                    .map(|(k, v)| (k.clone(), v.into()))
                    .collect(),
            }
        }
    }
}

pub mod raw {
    use crate::config;

    use dynconf::*;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    use anyhow::{anyhow, Result};

    #[derive(Deserialize, Serialize, Clone)]
    #[serde(deny_unknown_fields)]
    struct Repo {
        source: Option<String>,
        branch: Option<String>,
        manual: Option<bool>,
        path: Option<util::DynPath>,
    }

    #[derive(Deserialize, Serialize, Clone)]
    #[serde(transparent)]
    pub struct Repos {
        repos: HashMap<String, Repo>,
    }

    #[async_trait::async_trait]
    impl util::DynValue for Repo {
        type Target = super::Repo;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            let dynobj = config::utils::get_dyn_object(state)?;
            let repo_id = dynobj._id.ok_or_else(|| anyhow!("No _id binding"))?;

            let repo_dir = format!(
                "{}_{}",
                dynobj
                    .project
                    .ok_or_else(|| anyhow!("No project binding"))?
                    .id,
                repo_id,
            );
            let default_path = dynobj
                .config
                .ok_or_else(|| anyhow!("No service config binding"))?
                .repos_path
                .join(repo_dir)
                .to_string_lossy()
                .to_string();

            let path = self.path.load(state).await?.unwrap_or(default_path.into());

            let commit = crate::git::current_commit(path.clone()).await.ok();

            if !self.manual.unwrap_or(false) {
                Ok(super::Repo::Regular {
                    id: repo_id,
                    source: self
                        .source
                        .ok_or_else(|| anyhow!("'source' must be specified for not manual repo"))?,
                    branch: self.branch.unwrap_or_else(|| String::from("master")),
                    commit,
                    path,
                })
            } else {
                Ok(super::Repo::Manual { id: repo_id, path })
            }
        }
    }

    #[async_trait::async_trait]
    impl util::DynValue for Repos {
        type Target = super::Repos;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            Ok(super::Repos {
                repos: self.repos.load(state).await?,
            })
        }
    }
}
