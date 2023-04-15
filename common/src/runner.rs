use std::collections::HashSet;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmptyResponse {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ErrorResponse {
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProjectsListResponse {
    pub projects: HashSet<String>,
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
