use std::collections::{HashMap, HashSet, LinkedList};
use std::path::PathBuf;
use std::sync::Arc;

use crate::docker::Docker;
use crate::integrations::*;
use crate::tasks::{self, Task};

use common::Pipeline;

use common::run_context::RunContext;
use common::state::State;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, OwnedRwLockWriteGuard};

use serde::{Deserialize, Serialize};

use anyhow::anyhow;
use log::*;

pub struct Executor {
    pub runs: Mutex<Runs>,
    locks: Mutex<super::locks::Locks>,
}

pub const DEFEAULT_STAGE: &str = "default";

impl Executor {
    async fn run_stage(
        &self,
        pipeline: impl AsRef<str>,
        stage: impl AsRef<str>,
        strategy: common::OverlapStrategy,
        repos: &ReposList,
        lock_repos: &Option<common::StageRepos>,
    ) -> super::locks::StageGuard {
        let mut locks = self.locks.lock().await;
        locks
            .run_stage(pipeline, stage, strategy, repos, lock_repos)
            .await
    }
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
    pub stage: Mutex<Option<String>>,
    pub status: Mutex<PipelineStatus>,
    pub jobs: Mutex<HashMap<String, PipelineJob>>,
    pub log_file: Arc<Mutex<Option<tokio::fs::File>>>,
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
    Finished { error: Option<String> },
}

#[derive(Clone)]
pub enum PipelineFinishedStatus {
    Success,
    Error { message: String },
}

#[derive(Serialize, Deserialize)]
pub struct LogLine {
    #[serde(with = "chrono::serde::ts_milliseconds")]
    time: chrono::DateTime<chrono::Utc>,
    text: String,
    level: LogLevel,
    pipeline: Option<String>,
    job: Option<String>,
}

impl LogLine {
    pub fn new(
        text: String,
        level: LogLevel,
        pipeline: Option<String>,
        job: Option<String>,
    ) -> LogLine {
        LogLine {
            text,
            level,
            time: chrono::Utc::now(),
            pipeline,
            job,
        }
    }
}

pub struct Logger<'a> {
    pipeline_id: String,
    job_id: String,
    log_file: Arc<Mutex<Option<tokio::fs::File>>>,
    run_context: &'a RunContext,
}

impl<'a> Logger<'a> {
    pub async fn new<'b>(state: &'b State<'a>) -> Result<Logger<'a>, anyhow::Error>
    where
        'b: 'a,
    {
        Logger::new_impl(state, true).await
    }

    pub async fn new_no_log_file<'b>(state: &'b State<'a>) -> Result<Logger<'a>, anyhow::Error>
    where
        'b: 'a,
    {
        Logger::new_impl(state, false).await
    }

    async fn new_impl<'b>(
        state: &'b State<'a>,
        write_log: bool,
    ) -> Result<Logger<'a>, anyhow::Error>
    where
        'b: 'a,
    {
        let job_id: String = state.get_named("job").cloned()?;
        let run_context: &RunContext = state.get()?;
        let pipeline_run: &PipelineRun = state.get()?;
        let log_file = if write_log {
            pipeline_run.log_file.clone()
        } else {
            Arc::new(Mutex::new(None))
        };
        Ok(Logger {
            job_id,
            log_file,
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
        if let Some(log_file) = self.log_file.lock().await.as_mut() {
            log_file.write_all(log_line_text.as_bytes()).await?;
        }
        Ok(())
    }

    async fn log_impl(&mut self, text: String, level: LogLevel) -> Result<(), anyhow::Error> {
        self.log(LogLine::new(
            text,
            level,
            Some(self.pipeline_id.clone()),
            Some(self.job_id.clone()),
        ))
        .await
    }

    pub async fn error(&mut self, text: String) -> Result<(), anyhow::Error> {
        self.log_impl(text, LogLevel::Error).await
    }

    pub async fn regular(&mut self, text: String) -> Result<(), anyhow::Error> {
        self.log_impl(text, LogLevel::Regular).await
    }

    pub async fn warning(&mut self, text: String) -> Result<(), anyhow::Error> {
        self.log_impl(text, LogLevel::Warning).await
    }

    pub async fn heartbeat(&mut self) -> Result<(), anyhow::Error> {
        self.run_context.heartbeat().await;
        Ok(())
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

    pub async fn logs<'a>(
        &self,
        project: impl AsRef<str>,
        pipeline: impl AsRef<str>,
        run_id: impl AsRef<str>,
    ) -> Result<
        impl futures::Stream<Item = Result<common::runner::PipelineMessage, anyhow::Error>>,
        anyhow::Error,
    > {
        let log_file = if let Some(project) = self.get_project_runs(project.as_ref()) {
            if let Some(pipeline) = project.get_pipeline_runs(pipeline.as_ref()) {
                pipeline.get_log_file(run_id).await?
            } else {
                return Err(anyhow!("No such pipeline {}", pipeline.as_ref()));
            }
        } else {
            return Err(anyhow!("No such project {}", project.as_ref()));
        };
        let log_file = BufReader::new(log_file);

        let mut lines = log_file.lines();

        #[rustfmt::skip]
        let s = async_stream::try_stream! {
            while let Some(line) = lines.next_line().await? {
                if let Some(log) = parse_log(line)? {
                    yield log;
                }
            }
        };

        Ok(s)
    }
}

fn parse_log(log: String) -> Result<Option<common::runner::PipelineMessage>, anyhow::Error> {
    let log: LogLine = serde_json::from_str(&log)?;
    if let Some(pipeline) = log.pipeline {
        if let Some(job_id) = log.job {
            let t = match log.level {
                LogLevel::Regular => common::runner::LogType::Regular,
                LogLevel::Error => common::runner::LogType::Error,
                LogLevel::Warning => common::runner::LogType::Warning,
            };
            return Ok(Some(common::runner::PipelineMessage::Log {
                pipeline,
                job_id,
                t,
                text: log.text,
                timestamp: log.time,
            }));
        }
    }
    Ok(None)
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

    fn get_log_filename(&self, run_id: &str) -> PathBuf {
        PathBuf::from(RUNS_LOGS_DIR).join(format!("{}-{}.log", run_id, self.pipeline_id))
    }

    pub async fn get_log_file(
        &self,
        run_id: impl AsRef<str>,
    ) -> Result<tokio::fs::File, anyhow::Error> {
        if !self.runs.contains_key(run_id.as_ref()) {
            return Err(anyhow!("No such run {}", run_id.as_ref()));
        }

        Ok(tokio::fs::File::open(self.get_log_filename(run_id.as_ref())).await?)
    }

    pub async fn init_run(
        &mut self,
        run_id: impl AsRef<str>,
    ) -> Result<Arc<PipelineRun>, anyhow::Error> {
        let log_path = self.get_log_filename(run_id.as_ref());

        if self.runs_queue.len() >= self.queue_limit {
            let run_to_delete = self.runs_queue.pop_front().unwrap();
            self.runs.remove(&run_to_delete);
            let run_log_path = self.get_log_filename(&run_to_delete);
            tokio::fs::remove_file(run_log_path).await?;
        }

        let log_file = tokio::fs::File::create(log_path).await?;
        let run = Arc::new(PipelineRun::new(
            run_id.as_ref().to_string(),
            self.pipeline_id.clone(),
            log_file,
        ));
        self.runs_queue.push_back(run_id.as_ref().to_string());
        self.runs.insert(run_id.as_ref().to_string(), run.clone());

        Ok(run)
    }

    pub fn get_runs(&self) -> Vec<Arc<PipelineRun>> {
        self.runs.iter().map(|(k, v)| v.clone()).collect()
    }

    pub fn last_run(&self) -> Option<Arc<PipelineRun>> {
        let last_id = self.runs_queue.iter().last()?;
        self.runs.get(last_id).cloned()
    }
}

impl PipelineRun {
    pub fn new(id: String, pipeline_id: String, log_file: tokio::fs::File) -> Self {
        let started = chrono::Utc::now();
        Self {
            pipeline_id,
            id,
            started,
            status: Mutex::new(PipelineStatus::Starting),
            jobs: Mutex::new(HashMap::default()),
            log_file: Arc::new(Mutex::new(Some(log_file))),
            stage: Mutex::new(None),
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

    pub async fn job(&self, job: impl AsRef<str>) -> Option<PipelineJob> {
	self.jobs.lock().await.get(job.as_ref()).cloned()
    }

    pub async fn finish(&self) -> Result<(), anyhow::Error> {
        self.log_file.lock().await.take();
        Ok(())
    }

    pub async fn set_stage(&self, stage: String) {
        *self.stage.lock().await = Some(stage);
    }

    pub async fn stage(&self) -> Option<String> {
        self.stage.lock().await.clone()
    }
}

impl Default for PipelineJob {
    fn default() -> Self {
        Self {
            status: JobStatus::Pending,
        }
    }
}

pub struct ReposList {
    pub project: String,
    pub repos: Vec<String>,
}

impl Executor {
    pub async fn new() -> Result<Executor, anyhow::Error> {
        Ok(Executor {
            runs: Mutex::new(Runs::init().await?),
            locks: Mutex::new(super::locks::Locks::default()),
        })
    }

    pub async fn write_repo(
        &self,
        project_id: impl AsRef<str>,
        repo_id: impl AsRef<str>,
    ) -> Option<OwnedRwLockWriteGuard<()>> {
        self.locks
            .lock()
            .await
            .write_repo(project_id, repo_id)
            .await
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

        let integrations = Integrations::from_map(pipeline.integrations.clone())?;

        let pipeline_run: Arc<PipelineRun> = self
            .runs
            .lock()
            .await
            .init_run(project, pipeline.id.clone(), run_context.id.clone())
            .await?;

        let mut state = state.clone();
        state.set(pipeline_run.as_ref());
        state.set(&integrations);

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

        match res.as_ref() {
            Ok(_) => {
                integrations.handle_pipeline_done(&state).await;
            }
            Err(err) => {
                integrations
                    .handle_pipeline_fail(&state, Some(err.to_string()))
                    .await;
            }
        }
        pipeline_run.finish().await?;

        res
    }

    pub async fn run_impl<'a>(
        &self,
        state: &State<'a>,
        mut pipeline: Pipeline,
    ) -> Result<(), anyhow::Error> {
        info!("Running execution");
        let pipeline_run: &PipelineRun = state.get()?;
        let run_context: &RunContext = state.get()?;
        let integrations: &Integrations = state.get()?;

        let repos: &ReposList = state.get()?;

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

        let pop_ready = |deps: &mut HashMap<String, HashSet<String>>,
                         pipeline: &mut Pipeline|
         -> Vec<(String, common::Job)> {
            let res: Vec<String> = deps
                .iter()
                .filter(|(_, froms)| froms.is_empty())
                .map(|(k, _)| k.clone())
                .collect();
            for j in res.iter() {
                deps.remove(j);
            }
            res.into_iter()
                .map(|id| (id.clone(), pipeline.jobs.remove(&id).unwrap()))
                .collect()
        };

        integrations.handle_pipeline_start(&state).await;
        pipeline_run.set_status(PipelineStatus::Running).await;
        run_context
            .send(common::runner::PipelineMessage::Start {
                pipeline: pipeline.id.clone(),
            })
            .await;

        for (job_id, _) in pipeline.jobs.iter() {
            integrations.handle_job_pending(&state, &job_id).await;
            pipeline_run.init_job(job_id).await;
            run_context
                .send(common::runner::PipelineMessage::JobPending {
                    pipeline: pipeline.id.clone(),
                    job_id: job_id.clone(),
                })
                .await;
        }

        let mut stage_guard: Option<super::locks::StageGuard> = None;
        let mut was_stages: HashSet<String> = HashSet::new();

        let mut futs: FuturesUnordered<_> = FuturesUnordered::new();

        // NOTE: Do not check iterrupted here, because it's very
        // unlikely to be interrupted within this loop. The same for
        // inner loop below
        for (id, job) in pop_ready(&mut deps, &mut pipeline) {
            if let Some(stage_id) = job.stage.as_ref() {
                pipeline_run.set_stage(stage_id.to_string()).await;
                if let Some(stage) = pipeline.stages.get(stage_id) {
                    if !was_stages.insert(stage_id.to_string()) {
                        warn!("Trying enter stage {} twice, ignoring", stage_id);
                    } else {
                        stage_guard = Some(
                            self.run_stage(
                                &pipeline.id,
                                stage_id,
                                stage.overlap_strategy.clone(),
                                &repos,
                                &stage.repos,
                            )
                            .await,
                        );
                    }
                }
            }

            futs.push(self.run_job(&state, id, job));
        }

        while let Some(id) = futs.next().await {
            if let Some(stage_guard) = stage_guard.as_ref() {
                if stage_guard.interrupted().await {
                    warn!("Run was displaced");
                    break;
                }
            }

            let id = id?;
            for (_, wait_for) in deps.iter_mut() {
                wait_for.remove(&id);
            }

            for (id, job) in pop_ready(&mut deps, &mut pipeline) {
                if let Some(stage_id) = job.stage.as_ref() {
                    pipeline_run.set_stage(stage_id.to_string()).await;
                    if let Some(stage) = pipeline.stages.get(stage_id) {
                        if !was_stages.insert(stage_id.to_string()) {
                            warn!("Trying enter stage {} twice, ignoring", stage_id);
                        } else {
                            stage_guard = Some(
                                self.run_stage(
                                    &pipeline.id,
                                    stage_id,
                                    stage.overlap_strategy.clone(),
                                    &repos,
                                    &stage.repos,
                                )
                                .await,
                            );
                        }
                    }
                }

                futs.push(self.run_job(&state, id, job));
            }
        }

        // Wait the reset if was interrupted
        while let Some(_) = futs.next().await {}

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
        let integrations: &Integrations = state.get()?;

        let mut state = state.clone();
        state.set_named("job", &id);

        info!("Runnig job {}", id);
        let pipeline_run: &PipelineRun = state.get()?;

        for (i, step) in job.steps.into_iter().enumerate() {
	    if !job.enabled {
		break;
	    }

            integrations.handle_job_progress(&state, &id, i).await;
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

            if let Err(err) = step.run(&state).await {
                integrations
                    .handle_job_done(&state, &id, Some(err.to_string()))
                    .await;
                pipeline_run
                    .set_job_status(
                        &id,
                        JobStatus::Finished {
                            error: Some(err.to_string()),
                        },
                    )
                    .await;
                run_context
                    .send(common::runner::PipelineMessage::JobFinished {
                        pipeline: pipeline_run.pipeline_id.clone(),
                        job_id: id.clone(),
                        error: Some(err.to_string()),
                    })
                    .await;

		return Err(err);
            }
        }

        integrations.handle_job_done(&state, &id, None).await;
        pipeline_run
            .set_job_status(&id, JobStatus::Finished { error: None })
            .await;
        run_context
            .send(common::runner::PipelineMessage::JobFinished {
                pipeline: pipeline_run.pipeline_id.clone(),
                job_id: id.clone(),
                error: None,
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
            docker.create_volume_if_missing(volume).await?;
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
