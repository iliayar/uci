use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::context::Context;
use super::tasks::{self, Task};

use common::Pipeline;

use futures::stream::FuturesUnordered;
use futures::StreamExt;

use anyhow::anyhow;
use log::*;
use thiserror::Error;

pub struct Executor {
    context: Context,
}

#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("Task #{1} failed: {0}")]
    TaskError(tasks::TaskError, usize),

    #[error("IO Error: {0}")]
    IOError(#[from] tokio::io::Error),

    #[error("Docker error: {0}")]
    DockerError(#[from] crate::docker::DockerError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Executor {
    pub fn new(context: Context) -> Result<Executor, ExecutorError> {
        Ok(Executor { context })
    }

    pub async fn run(self, config: Pipeline) {
        debug!("Running pipeline: {:?}", config);
        if let Err(err) = self.run_impl(config).await {
            error!("Executor failed: {}", err);
        }
    }

    async fn make_task_context(
        &self,
        config: &Pipeline,
    ) -> Result<tasks::TaskContext, ExecutorError> {
        info!("Creating task context");
        let links: HashMap<_, _> = config
            .links
            .iter()
            .map(|(k, v)| (k.clone(), PathBuf::from(v)))
            .collect();

        for (_, path) in links.iter() {
            tokio::fs::create_dir_all(path).await?;
        }

        Ok(tasks::TaskContext { links })
    }

    pub async fn run_impl(self, config: Pipeline) -> Result<(), ExecutorError> {
        info!("Running execution");

        let task_context = self.make_task_context(&config).await?;

        let mut deps: HashMap<String, HashSet<String>> = config
            .jobs
            .iter()
            .map(|(k, v)| (k.clone(), v.needs.iter().cloned().collect()))
            .collect();

        if cycles::check(&deps) {
            return Err(anyhow!("Jobs contains a dependencies cycle, do not run anything").into());
        }

        self.ensure_resources_exists(&config.networks, &config.volumes)
            .await?;

        let pop_ready =
            |deps: &mut HashMap<String, HashSet<String>>| -> Vec<(String, common::Job)> {
                let res: Vec<String> = deps
                    .iter()
                    .filter(|(_, froms)| froms.is_empty())
                    .map(|(k, _)| k.clone())
                    .collect();
                for j in res.iter() {
                    deps.remove(j);
                }
                res.into_iter()
                    .map(|id| (id.clone(), config.jobs.get(&id).unwrap().clone()))
                    .collect()
            };

        let mut futs: FuturesUnordered<_> = FuturesUnordered::new();

        for (id, job) in pop_ready(&mut deps) {
            futs.push(self.run_job(id, job, &task_context));
        }

        while let Some(id) = futs.next().await {
            let id = id?;
            for (_, wait_for) in deps.iter_mut() {
                wait_for.remove(&id);
            }

            for (id, job) in pop_ready(&mut deps) {
                futs.push(self.run_job(id, job, &task_context));
            }
        }

        info!("All jobs done");

        Ok(())
    }

    async fn run_job(
        &self,
        id: String,
        job: common::Job,
        task_context: &tasks::TaskContext,
    ) -> Result<String, ExecutorError> {
        info!("Runnig job {}", id);

        for (i, step) in job.steps.into_iter().enumerate() {
            step.run(&self.context, task_context)
                .await
                .map_err(|e| ExecutorError::TaskError(e, i))?;
        }

        info!("Job {} done", id);

        Ok(id)
    }

    async fn ensure_resources_exists(
        &self,
        networks: &[String],
        volumes: &[String],
    ) -> Result<(), ExecutorError> {
        for network in networks {
            self.context
                .docker()
                .create_network_if_missing(&network)
                .await?;
        }

        for volume in volumes {
            self.context
                .docker()
                .create_network_if_missing(&volume)
                .await?;
        }

        Ok(())
    }
}

mod cycles {
    use std::collections::{HashMap, HashSet};

    use log::*;

    enum State {
        NotVisited,
        InProgress,
        Visited,
    }

    impl Default for &State {
        fn default() -> Self {
            &State::NotVisited
        }
    }

    pub fn check(deps: &HashMap<String, HashSet<String>>) -> bool {
        let mut was = HashMap::new();

        for (id, _) in deps.iter() {
            if let State::NotVisited = was.get(id).unwrap_or_default() {
                if dfs(id.clone(), deps, &mut was) {
                    return true;
                }
            }
        }

        return false;
    }

    fn dfs(
        cur: String,
        deps: &HashMap<String, HashSet<String>>,
        was: &mut HashMap<String, State>,
    ) -> bool {
        if let State::InProgress = was.get(&cur).unwrap_or_default() {
            warn!("Found cycle with job {}", cur);
            return true;
        }

        was.insert(cur.clone(), State::InProgress);
        for to in deps.get(&cur).unwrap() {
            if dfs(to.clone(), deps, was) {
                return true;
            }
        }
        was.insert(cur, State::Visited);

        return false;
    }
}
