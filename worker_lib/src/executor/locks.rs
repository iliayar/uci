use std::{collections::HashMap, sync::Arc};

use tokio::sync::{Mutex, OwnedMutexGuard, OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

// FIXME: now it's just like arc mutex them all. feels wrong
#[derive(Default)]
pub struct Locks {
    pipelines: HashMap<String, PipelineLocks>,
    project_repos: HashMap<String, ProjectRepos>,
}

#[derive(Default)]
struct ProjectRepos {
    repos: HashMap<String, Arc<RwLock<()>>>,
}

#[derive(Default)]
struct PipelineLocks {
    stages: HashMap<String, StageLock>,
}

struct StageLock {
    lock: Arc<Mutex<()>>,
    interrupted: Arc<Mutex<bool>>,
}

pub struct StageGuard {
    guard: Option<OwnedMutexGuard<()>>,
    interrupted: Option<Arc<Mutex<bool>>>,
    repos_guard: Vec<OwnedRwLockReadGuard<()>>,
}

impl StageGuard {
    pub async fn interrupted(&self) -> bool {
        if let Some(interrupted) = self.interrupted.as_ref() {
            *interrupted.lock().await
        } else {
            false
        }
    }
}

impl Locks {
    pub async fn write_repo(
        &self,
        project_id: impl AsRef<str>,
        repo_id: impl AsRef<str>,
    ) -> Option<OwnedRwLockWriteGuard<()>> {
        if let Some(project) = self.project_repos.get(project_id.as_ref()) {
            if let Some(repo) = project.repos.get(repo_id.as_ref()) {
                return Some(repo.clone().write_owned().await);
            }
        }

        None
    }

    pub async fn run_stage(
        &mut self,
        pipeline: impl AsRef<str>,
        stage: impl AsRef<str>,
        strategy: common::OverlapStrategy,
        repos: &super::core::ReposList,
        lock_repos: &Option<common::StageRepos>,
    ) -> StageGuard {
        let repos_guard = self.get_repo_locks(repos, lock_repos).await;
        match strategy {
            common::OverlapStrategy::Ignore => {
                return StageGuard {
                    guard: None,
                    interrupted: None,
                    repos_guard,
                }
            }
            common::OverlapStrategy::Displace => {
                let stage_lock = self.get_stage_lock(pipeline, stage);
                let interrupted = stage_lock.interrupted.clone();

                *interrupted.lock().await = true;

                // FIXME: Some races here?
                let lock_guard = stage_lock.lock.clone().lock_owned().await;
                *interrupted.lock().await = false;

                StageGuard {
                    guard: Some(lock_guard),
                    interrupted: Some(interrupted),
                    repos_guard,
                }
            }
            common::OverlapStrategy::Wait => {
                let stage_lock = self.get_stage_lock(pipeline, stage);
                let lock_guard = stage_lock.lock.clone().lock_owned().await;
                StageGuard {
                    guard: Some(lock_guard),
                    interrupted: None,
                    repos_guard,
                }
            }
        }
    }

    fn get_stage_lock(&mut self, pipeline: impl AsRef<str>, stage: impl AsRef<str>) -> &StageLock {
        if !self.pipelines.contains_key(pipeline.as_ref()) {
            self.pipelines
                .insert(pipeline.as_ref().to_string(), PipelineLocks::default());
        }

        let pipeline_locks = self.pipelines.get_mut(pipeline.as_ref()).unwrap();

        if !pipeline_locks.stages.contains_key(stage.as_ref()) {
            pipeline_locks.stages.insert(
                stage.as_ref().to_string(),
                StageLock {
                    lock: Arc::new(Mutex::new(())),
                    interrupted: Arc::new(Mutex::new(false)),
                },
            );
        }

        pipeline_locks.stages.get(stage.as_ref()).as_ref().unwrap()
    }

    async fn get_repo_locks(
        &mut self,
        repos: &super::core::ReposList,
        lock_repos: &Option<common::StageRepos>,
    ) -> Vec<OwnedRwLockReadGuard<()>> {
        if !self.project_repos.contains_key(&repos.project) {
            self.project_repos
                .insert(repos.project.clone(), ProjectRepos::default());
        }

        if let Some(lock_repos) = lock_repos {
            let project = self.project_repos.get_mut(&repos.project).unwrap();

            let mut locks = Vec::new();
            for repo in repos.repos.iter() {
                if !project.repos.contains_key(repo) {
                    project
                        .repos
                        .insert(repo.clone(), Arc::new(RwLock::new(())));
                }

                locks.push(project.repos.get(repo).unwrap().clone().read_owned().await);
            }
            locks
        } else {
            Vec::new()
        }
    }
}
