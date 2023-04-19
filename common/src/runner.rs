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
