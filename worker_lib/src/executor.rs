use std::collections::{HashMap, HashSet, LinkedList};
use std::path::PathBuf;
use std::sync::Arc;

use crate::docker::Docker;

use super::tasks::{self, Task};

use common::Pipeline;

use common::run_context::RunContext;
use common::state::State;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use serde::{Deserialize, Serialize};

use anyhow::anyhow;
use log::*;

pub struct Executor {
    pub runs: Mutex<Runs>,
}

const RUNS_LOGS_DIR: &str = "/tmp/uci-runs";

pub struct Runs {
    projects: HashMap<String, ProjectRuns>,
}

#[derive(Default)]
pub struct ProjectRuns {
    pipelines: HashMap<String, PipelineRuns>,
}

pub struct PipelineRuns {
    pipeline_id: String,
    queue_limit: usize,
    runs: HashMap<String, Arc<PipelineRun>>,
    runs_queue: LinkedList<String>,
}

pub struct PipelineRun {
    pub pipeline_id: String,
    pub id: String,
    pub started: chrono::DateTime<chrono::Utc>,
    pub status: Mutex<PipelineStatus>,
    pub jobs: Mutex<HashMap<String, PipelineJob>>,
}

#[derive(Clone)]
pub enum PipelineStatus {
    Starting,
    Running,
    Finished(PipelineFinishedStatus),
}

#[derive(Clone)]
pub struct PipelineJob {
    pub status: JobStatus,
}

#[derive(Clone)]
pub enum JobStatus {
    Pending,
    Running { step: usize },
    Finished,
}

#[derive(Clone)]
pub enum PipelineFinishedStatus {
    Success,
    Error { message: String },
}

#[derive(Serialize, Deserialize)]
pub struct LogLine {
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub time: chrono::DateTime<chrono::Utc>,
    pub text: String,
    pub level: LogLevel,
}

impl LogLine {
    pub fn error(text: String) -> LogLine {
        LogLine::new(text, LogLevel::Error)
    }

    pub fn regular(text: String) -> LogLine {
        LogLine::new(text, LogLevel::Regular)
    }

    pub fn warning(text: String) -> LogLine {
        LogLine::new(text, LogLevel::Warning)
    }

    pub fn new(text: String, level: LogLevel) -> LogLine {
        LogLine {
            text,
            level,
            time: chrono::Utc::now(),
        }
    }
}

pub struct Logger<'a> {
    pipeline_id: String,
    job_id: String,
    log_file: tokio::fs::File,
    write_file: bool,
    run_context: &'a RunContext,
}

impl<'a> Logger<'a> {
    pub async fn new<'b>(state: &'b State<'a>) -> Result<Logger<'a>, anyhow::Error>
    where
        'b: 'a,
    {
        let job_id: String = state.get_named("job").cloned()?;
        let run_context: &RunContext = state.get()?;
        let pipeline_run: &PipelineRun = state.get()?;
        let log_file = pipeline_run.job_log_file(&job_id).await?;
        Ok(Logger {
            job_id,
            log_file,
            write_file: true,
            run_context,
            pipeline_id: pipeline_run.pipeline_id.clone(),
        })
    }

    pub async fn log(&mut self, log: LogLine) -> Result<(), anyhow::Error> {
        self.run_context
            .send(common::runner::PipelineMessage::Log {
                t: match log.level {
                    LogLevel::Regular => common::runner::LogType::Regular,
                    LogLevel::Error => common::runner::LogType::Error,
                    LogLevel::Warning => common::runner::LogType::Warning,
                },
                text: log.text.clone(),
                timestamp: log.time,
                pipeline: self.pipeline_id.clone(),
                job_id: self.job_id.clone(),
            })
            .await;

        let mut log_line_text = serde_json::to_string(&log)?;
        debug!("{}: {}", self.job_id, log_line_text);
        log_line_text.push('\n');
        self.log_file.write_all(log_line_text.as_bytes()).await?;
        Ok(())
    }

    pub async fn error(&mut self, text: String) -> Result<(), anyhow::Error> {
        self.log(LogLine::error(text)).await
    }

    pub async fn regular(&mut self, text: String) -> Result<(), anyhow::Error> {
        self.log(LogLine::regular(text)).await
    }

    pub async fn warning(&mut self, text: String) -> Result<(), anyhow::Error> {
        self.log(LogLine::warning(text)).await
    }

    pub fn write_file(&mut self, value: bool) {
        self.write_file = value;
    }
}

#[derive(Serialize, Deserialize)]
pub enum LogLevel {
    Regular,
    Error,
    Warning,
}

impl Runs {
    pub async fn init() -> Result<Self, anyhow::Error> {
        let path: PathBuf = RUNS_LOGS_DIR.into();

        if path.exists() {
            tokio::fs::remove_dir_all(&path).await?;
        }

        tokio::fs::create_dir_all(&path).await?;

        Ok(Self {
            projects: Default::default(),
        })
    }

    pub fn get_project_runs(&self, project: impl AsRef<str>) -> Option<&ProjectRuns> {
        self.projects.get(project.as_ref())
    }

    pub fn get_projects(&self) -> Vec<String> {
        self.projects.iter().map(|(k, v)| k.clone()).collect()
    }

    pub async fn init_run(
        &mut self,
        project: impl AsRef<str>,
        pipeline: impl AsRef<str>,
        run_id: impl AsRef<str>,
    ) -> Result<Arc<PipelineRun>, anyhow::Error> {
        if !self.projects.contains_key(project.as_ref()) {
            self.projects
                .insert(project.as_ref().to_string(), ProjectRuns::default());
        }

        self.projects
            .get_mut(project.as_ref())
            .unwrap()
            .init_run(pipeline, run_id)
            .await
    }
}

impl ProjectRuns {
    pub async fn init_run(
        &mut self,
        pipeline: impl AsRef<str>,
        run_id: impl AsRef<str>,
    ) -> Result<Arc<PipelineRun>, anyhow::Error> {
        if !self.pipelines.contains_key(pipeline.as_ref()) {
            self.pipelines.insert(
                pipeline.as_ref().to_string(),
                PipelineRuns::new(pipeline.as_ref().to_string()).await,
            );
        }

        self.pipelines
            .get_mut(pipeline.as_ref())
            .unwrap()
            .init_run(run_id)
            .await
    }

    pub fn get_pipeline_runs(&self, pipeline: impl AsRef<str>) -> Option<&PipelineRuns> {
        self.pipelines.get(pipeline.as_ref())
    }

    pub fn get_pipelines(&self) -> Vec<String> {
        self.pipelines.iter().map(|(k, v)| k.clone()).collect()
    }
}

impl PipelineRuns {
    pub async fn new(pipeline_id: String) -> Self {
        Self {
            pipeline_id,
            queue_limit: 1,
            runs: Default::default(),
            runs_queue: Default::default(),
        }
    }

    pub async fn init_run(
        &mut self,
        run_id: impl AsRef<str>,
    ) -> Result<Arc<PipelineRun>, anyhow::Error> {
        let run_logs_dir = PathBuf::from(RUNS_LOGS_DIR)
            .join(run_id.as_ref())
            .join(&self.pipeline_id);
        tokio::fs::create_dir_all(run_logs_dir).await?;

        if self.runs_queue.len() >= self.queue_limit {
            let run_to_delete = self.runs_queue.pop_front().unwrap();
            self.runs.remove(&run_to_delete);
            let run_logs_dir = PathBuf::from(RUNS_LOGS_DIR)
                .join(&run_to_delete)
                .join(&self.pipeline_id);
            tokio::fs::remove_dir_all(run_logs_dir).await?;

            let run_logs_dir = PathBuf::from(RUNS_LOGS_DIR).join(&run_to_delete);

            if let Ok(mut dir) = run_logs_dir.read_dir() {
                // Is empty
                if !dir.any(|_| true) {
                    tokio::fs::remove_dir(run_logs_dir).await?;
                }
            }
        }

        let run = Arc::new(PipelineRun::new(
            run_id.as_ref().to_string(),
            self.pipeline_id.clone(),
        ));
        self.runs_queue.push_back(run_id.as_ref().to_string());
        self.runs.insert(run_id.as_ref().to_string(), run.clone());

        Ok(run)
    }

    pub fn get_runs(&self) -> Vec<Arc<PipelineRun>> {
        self.runs.iter().map(|(k, v)| v.clone()).collect()
    }
}

impl PipelineRun {
    pub fn new(id: String, pipeline_id: String) -> Self {
        let started = chrono::Utc::now();
        Self {
            pipeline_id,
            id,
            started,
            status: Mutex::new(PipelineStatus::Starting),
            jobs: Mutex::new(HashMap::default()),
        }
    }

    pub async fn set_status(&self, status: PipelineStatus) {
        *self.status.lock().await = status;
    }

    pub async fn status(&self) -> PipelineStatus {
        self.status.lock().await.clone()
    }

    pub async fn init_job(&self, job: impl AsRef<str>) {
        self.jobs
            .lock()
            .await
            .insert(job.as_ref().to_string(), PipelineJob::default());
    }

    pub async fn set_job_status(&self, job: impl AsRef<str>, status: JobStatus) {
        if let Some(job) = self.jobs.lock().await.get_mut(job.as_ref()) {
            job.status = status;
        }
    }

    pub async fn jobs(&self) -> HashMap<String, PipelineJob> {
        self.jobs.lock().await.clone()
    }

    pub async fn job_log_file(
        &self,
        job: impl AsRef<str>,
    ) -> Result<tokio::fs::File, anyhow::Error> {
        let log_path = PathBuf::from(RUNS_LOGS_DIR)
            .join(&self.id)
            .join(&self.pipeline_id)
            .join(format!("{}.log", job.as_ref()));

        let mut options = tokio::fs::OpenOptions::new();
        let file = options.append(true).create(true).open(log_path).await?;

        Ok(file)
    }
}

impl Default for PipelineJob {
    fn default() -> Self {
        Self {
            status: JobStatus::Pending,
        }
    }
}

impl Executor {
    pub async fn new() -> Result<Executor, anyhow::Error> {
        Ok(Executor {
            runs: Mutex::new(Runs::init().await?),
        })
    }

    pub async fn run<'a>(&self, state: &State<'a>, config: Pipeline) {
        debug!("Running pipeline: {:?}", config);
        if let Err(err) = self.run_impl_with_run(state, config).await {
            error!("Executor failed: {}", err);
        }
    }

    pub async fn run_result<'a>(
        &self,
        state: &State<'a>,
        config: Pipeline,
    ) -> Result<(), anyhow::Error> {
        self.run_impl_with_run(state, config).await
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

    pub async fn run_impl_with_run<'a>(
        &self,
        state: &State<'a>,
        pipeline: Pipeline,
    ) -> Result<(), anyhow::Error> {
        let run_context: &RunContext = state.get()?;
        let project: String = state.get_named("project").cloned()?;

        let pipeline_run: Arc<PipelineRun> = self
            .runs
            .lock()
            .await
            .init_run(project, pipeline.id.clone(), run_context.id.clone())
            .await?;

        let mut state = state.clone();
        state.set(pipeline_run.as_ref());

        let res = self.run_impl(&state, pipeline).await;

        let finished_status = match res.as_ref() {
            Ok(_) => PipelineFinishedStatus::Success,
            Err(err) => PipelineFinishedStatus::Error {
                message: err.to_string(),
            },
        };

        pipeline_run
            .set_status(PipelineStatus::Finished(finished_status))
            .await;

        let error = match res.as_ref() {
            Ok(_) => None,
            Err(err) => Some(err.to_string()),
        };

        run_context
            .send(common::runner::PipelineMessage::Finish {
                pipeline: pipeline_run.pipeline_id.clone(),
                error,
            })
            .await;

        res
    }

    pub async fn run_impl<'a>(
        &self,
        state: &State<'a>,
        pipeline: Pipeline,
    ) -> Result<(), anyhow::Error> {
        info!("Running execution");
        let pipeline_run: &PipelineRun = state.get()?;
        let run_context: &RunContext = state.get()?;

        let task_context = self.make_task_context(&pipeline).await?;

        let mut state = state.clone();
        state.set(&task_context);

        let mut deps: HashMap<String, HashSet<String>> = pipeline
            .jobs
            .iter()
            .map(|(k, v)| (k.clone(), v.needs.iter().cloned().collect()))
            .collect();

        if cycles::check(&deps) {
            return Err(anyhow!(
                "Jobs contains a dependencies cycle, do not run anything"
            ));
        }

        self.ensure_resources_exists(&state, &pipeline.networks, &pipeline.volumes)
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
                    .map(|id| (id.clone(), pipeline.jobs.get(&id).unwrap().clone()))
                    .collect()
            };

        pipeline_run.set_status(PipelineStatus::Running).await;
        run_context
            .send(common::runner::PipelineMessage::Start {
                pipeline: pipeline.id.clone(),
            })
            .await;

        for (job_id, _) in pipeline.jobs.iter() {
            pipeline_run.init_job(job_id).await;
            run_context
                .send(common::runner::PipelineMessage::JobPending {
                    pipeline: pipeline.id.clone(),
                    job_id: job_id.clone(),
                })
                .await;
        }

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
        let run_context: &RunContext = state.get()?;

        let mut state = state.clone();
        state.set_named("job", &id);

        info!("Runnig job {}", id);
        let pipeline_run: &PipelineRun = state.get()?;

        for (i, step) in job.steps.into_iter().enumerate() {
            pipeline_run
                .set_job_status(&id, JobStatus::Running { step: i })
                .await;
            run_context
                .send(common::runner::PipelineMessage::JobProgress {
                    pipeline: pipeline_run.pipeline_id.clone(),
                    job_id: id.clone(),
                    step: i,
                })
                .await;
            step.run(&state).await?
        }

        pipeline_run.set_job_status(&id, JobStatus::Finished).await;
        run_context
            .send(common::runner::PipelineMessage::JobFinished {
                pipeline: pipeline_run.pipeline_id.clone(),
                job_id: id.clone(),
            })
            .await;
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
