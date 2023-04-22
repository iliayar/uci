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
    pub service_id: String,
    pub follow: bool,
    pub tail: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateRepoQuery {
    pub project_id: String,
    pub repo_id: String,
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
pub struct Run {
    pub project: String,
    pub pipeline: String,
    pub run_id: String,

    #[serde(with = "chrono::serde::ts_seconds")]
    pub started: chrono::DateTime<chrono::Utc>,

    pub status: RunStatus,
    pub jobs: HashMap<String, Job>,
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
    Finished,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PipelineMessage {
    Start,
    Finish,
    Log {
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
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum LogType {
    Regular,
    Error,
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
