use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::docker::Docker;

use super::tasks::{self, Task};

use common::Pipeline;

use common::state::State;
use futures::stream::FuturesUnordered;
use futures::StreamExt;

use anyhow::anyhow;
use log::*;

pub struct Executor {}

impl Executor {
    pub fn new() -> Result<Executor, anyhow::Error> {
        Ok(Executor {})
    }

    pub async fn run<'a>(&self, state: &State<'a>, config: Pipeline) {
        debug!("Running pipeline: {:?}", config);
        if let Err(err) = self.run_impl(state, config).await {
            error!("Executor failed: {}", err);
        }
    }

    pub async fn run_result<'a>(
        &self,
        state: &State<'a>,
        config: Pipeline,
    ) -> Result<(), anyhow::Error> {
        self.run_impl(state, config).await
    }

    async fn make_task_context(
        &self,
        config: &Pipeline,
    ) -> Result<tasks::TaskContext, anyhow::Error> {
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

    pub async fn run_impl<'a>(
        &self,
        state: &State<'a>,
        config: Pipeline,
    ) -> Result<(), anyhow::Error> {
        info!("Running execution");
        let mut state = state.clone();

        let task_context = self.make_task_context(&config).await?;
        state.set(&task_context);

        let mut deps: HashMap<String, HashSet<String>> = config
            .jobs
            .iter()
            .map(|(k, v)| (k.clone(), v.needs.iter().cloned().collect()))
            .collect();

        if cycles::check(&deps) {
            return Err(anyhow!(
                "Jobs contains a dependencies cycle, do not run anything"
            ));
        }

        self.ensure_resources_exists(&state, &config.networks, &config.volumes)
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
            futs.push(self.run_job(&state, id, job));
        }

        while let Some(id) = futs.next().await {
            let id = id?;
            for (_, wait_for) in deps.iter_mut() {
                wait_for.remove(&id);
            }

            for (id, job) in pop_ready(&mut deps) {
                futs.push(self.run_job(&state, id, job));
            }
        }

        info!("All jobs done");

        Ok(())
    }

    async fn run_job<'a>(
        &self,
        state: &State<'a>,
        id: String,
        job: common::Job,
    ) -> Result<String, anyhow::Error> {
        info!("Runnig job {}", id);

        for (i, step) in job.steps.into_iter().enumerate() {
            step.run(state).await?
        }

        info!("Job {} done", id);

        Ok(id)
    }

    async fn ensure_resources_exists<'a>(
        &self,
        state: &State<'a>,
        networks: &[String],
        volumes: &[String],
    ) -> Result<(), anyhow::Error> {
        let docker: &Docker = state.get()?;
        for network in networks {
            docker.create_network_if_missing(network).await?;
        }

        for volume in volumes {
            docker.create_network_if_missing(volume).await?;
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

        false
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

        false
    }
}
