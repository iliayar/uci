use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmptyResponse {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ErrorResponse {
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProjectsListResponse {
    pub projects: Vec<Project>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ListReposQuery {
    pub project_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReposListResponse {
    pub repos: Vec<Repo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Repo {
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Project {
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActionsListResponse {
    pub actions: Vec<Action>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Action {
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ActionTrigger {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServicesListResponse {
    pub services: Vec<Service>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Service {
    pub id: String,
    pub status: ServiceStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServiceStatus {
    Running,
    Starting,
    NotRunning,
    Dead,
    Exited,
    Restarting,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PipelinesListResponse {
    pub pipelines: Vec<Pipeline>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Pipeline {
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContinueReponse {
    pub run_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum UpdateRepoMessage {
    PullingRepo,
    RepoPulled { changed_files: Vec<String> },
    WholeRepoUpdated,
    FailedToPull { err: String },
    NoSuchRepo,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CloneMissingRepos {
    Begin,
    ClonningRepo { repo_id: String },
    RepoCloned { repo_id: String },
    Finish,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CallRequest {
    pub project_id: String,
    pub trigger_id: String,
    pub dry_run: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceCommandRequest {
    pub project_id: String,
    pub services: Vec<String>,
    pub command: ServiceCommand,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServiceCommand {
    Stop,
    Start { build: bool },
    Restart { build: bool },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UploadResponse {
    pub artifact: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ListActionsQuery {
    pub project_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ListPipelinesQuery {
    pub project_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ListServicesQuery {
    pub project_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceLogsQuery {
    pub project_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceLogsBody {
    pub services: Vec<String>,
    pub follow: bool,
    pub tail: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateRepoBody {
    pub project_id: String,
    pub repo_id: String,
    pub artifact_id: Option<String>,
    pub dry_run: Option<bool>,
    pub update_only: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ListRunsRequestQuery {
    pub project_id: Option<String>,
    pub pipeline_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ListRunsResponse {
    pub runs: Vec<Run>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RunsLogsRequestQuery {
    pub run: String,
    pub project: String,
    pub pipeline: String,
    // TODO: Maybe add filters
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RunsCancelRequestBody {
    pub run: String,
    pub project: String,
    pub pipeline: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Run {
    pub project: String,
    pub pipeline: String,
    pub run_id: String,

    #[serde(with = "chrono::serde::ts_seconds")]
    pub started: chrono::DateTime<chrono::Utc>,

    pub status: RunStatus,
    pub jobs: HashMap<String, Job>,
    pub stage: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RunStatus {
    Running,
    Finished(RunFinishedStatus),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RunFinishedStatus {
    Success,
    Error { message: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Job {
    pub status: JobStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum JobStatus {
    Pending,
    Running { step: usize },
    Finished { error: Option<String> },
    Skipped,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PipelineMessage {
    Start {
        pipeline: String,
    },
    Finish {
        pipeline: String,
        error: Option<String>,
    },
    JobPending {
        pipeline: String,
        job_id: String,
    },
    JobProgress {
        pipeline: String,
        job_id: String,
        step: usize,
    },
    JobFinished {
        pipeline: String,
        job_id: String,
        error: Option<String>,
    },
    JobSkipped {
        pipeline: String,
        job_id: String,
    },
    Log {
        pipeline: String,
        job_id: String,
        t: LogType,
        text: String,
        #[serde(with = "chrono::serde::ts_milliseconds")]
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    ContainerLog {
        container: String,
        t: LogType,
        text: String,
        #[serde(with = "chrono::serde::ts_milliseconds")]
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    Heartbeat,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum LogType {
    Regular,
    Error,
    Warning,
}

impl AsRef<PipelineMessage> for PipelineMessage {
    fn as_ref(&self) -> &PipelineMessage {
        self
    }
}

impl AsRef<UpdateRepoMessage> for UpdateRepoMessage {
    fn as_ref(&self) -> &UpdateRepoMessage {
        self
    }
}

impl AsRef<CloneMissingRepos> for CloneMissingRepos {
    fn as_ref(&self) -> &CloneMissingRepos {
        self
    }
}
