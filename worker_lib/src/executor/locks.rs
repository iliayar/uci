use std::{collections::HashMap, sync::Arc};

use tokio::sync::{Mutex, OwnedMutexGuard, OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

// FIXME: now it's just like arc mutex them all. feels wrong
#[derive(Default)]
pub struct Locks {
    pipelines: Mutex<HashMap<String, PipelineLocks>>,
    project_repos: Mutex<HashMap<String, ProjectRepos>>,
}

#[derive(Default)]
struct ProjectRepos {
    repos: HashMap<String, Arc<RwLock<()>>>,
}

#[derive(Default)]
struct PipelineLocks {
    stages: HashMap<String, StageLock>,
}

#[derive(Clone, Copy)]
pub enum Interrupted {
    None,
    Displaced,
    Canceled,
}

#[derive(Clone)]
struct StageLock {
    lock: Arc<Mutex<()>>,
    interrupted: Arc<Mutex<Interrupted>>,
    current_run: Option<Arc<super::PipelineRun>>,
}

pub struct StageGuard {
    guard: Option<OwnedMutexGuard<()>>,
    interrupted: Option<Arc<Mutex<Interrupted>>>,
    repos_guard: Vec<OwnedRwLockReadGuard<()>>,
}

impl StageGuard {
    pub async fn interrupted(&self) -> Interrupted {
        if let Some(interrupted) = self.interrupted.as_ref() {
            *interrupted.lock().await
        } else {
            Interrupted::None
        }
    }
}

impl Locks {
    pub async fn write_repo(
        &self,
        project_id: impl AsRef<str>,
        repo_id: impl AsRef<str>,
    ) -> Option<OwnedRwLockWriteGuard<()>> {
        if let Some(project) = self.project_repos.lock().await.get(project_id.as_ref()) {
            if let Some(repo) = project.repos.get(repo_id.as_ref()) {
                return Some(repo.clone().write_owned().await);
            }
        }

        None
    }

    pub async fn run_stage(
        &self,
        pipeline: impl AsRef<str>,
        stage: impl AsRef<str>,
        run: Option<Arc<super::PipelineRun>>,
        strategy: common::OverlapStrategy,
        repos: &super::core::ReposList,
        lock_repos: &Option<common::StageRepos>,
    ) -> StageGuard {
        let repos_guard = self.get_repo_locks(repos, lock_repos).await;
        match &strategy {
            common::OverlapStrategy::Ignore => {
                return StageGuard {
                    guard: None,
                    interrupted: None,
                    repos_guard,
                }
            }
            common::OverlapStrategy::Displace | common::OverlapStrategy::Cancel => {
                let interrupted_type = match strategy {
                    common::OverlapStrategy::Displace => Interrupted::Displaced,
                    common::OverlapStrategy::Cancel => Interrupted::Canceled,
                    _ => unreachable!(),
                };

                let stage_lock = self.get_stage_lock(pipeline, stage, run).await;

                // FIXME: Some races here?
                let interrupted = stage_lock.interrupted.clone();

                *interrupted.lock().await = interrupted_type;

                if let Interrupted::Canceled = interrupted_type {
                    if let Some(run) = stage_lock.current_run.as_ref() {
                        run.cancel().await;
                    }
                }

                let lock_guard = stage_lock.lock.clone().lock_owned().await;

                *interrupted.lock().await = Interrupted::None;

                StageGuard {
                    guard: Some(lock_guard),
                    interrupted: Some(interrupted),
                    repos_guard,
                }
            }
            common::OverlapStrategy::Wait => {
                let stage_lock = self.get_stage_lock(pipeline, stage, run).await;
                let lock_guard = stage_lock.lock.clone().lock_owned().await;
                StageGuard {
                    guard: Some(lock_guard),
                    interrupted: None,
                    repos_guard,
                }
            }
        }
    }

    async fn get_stage_lock(
        &self,
        pipeline: impl AsRef<str>,
        stage: impl AsRef<str>,
        run: Option<Arc<super::PipelineRun>>,
    ) -> StageLock {
        let mut pipelines = self.pipelines.lock().await;
        if !pipelines.contains_key(pipeline.as_ref()) {
            pipelines.insert(pipeline.as_ref().to_string(), PipelineLocks::default());
        }

        let pipeline_locks = pipelines.get_mut(pipeline.as_ref()).unwrap();
        if !pipeline_locks.stages.contains_key(stage.as_ref()) {
            pipeline_locks.stages.insert(
                stage.as_ref().to_string(),
                StageLock {
                    lock: Arc::new(Mutex::new(())),
                    interrupted: Arc::new(Mutex::new(Interrupted::None)),
                    current_run: None,
                },
            );
        }

        let stage_lock = pipeline_locks.stages.get_mut(stage.as_ref()).unwrap();
        let result = stage_lock.clone();

        stage_lock.current_run = run;

        result
    }

    async fn get_repo_locks(
        &self,
        repos: &super::core::ReposList,
        lock_repos: &Option<common::StageRepos>,
    ) -> Vec<OwnedRwLockReadGuard<()>> {
        let mut project_repos = self.project_repos.lock().await;
        if !project_repos.contains_key(&repos.project) {
            project_repos.insert(repos.project.clone(), ProjectRepos::default());
        }

        if let Some(lock_repos) = lock_repos {
            let project = project_repos.get_mut(&repos.project).unwrap();

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
