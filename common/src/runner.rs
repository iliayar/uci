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
    RepoPulled { changed_files: Vec<String> },
    FailedToPull { err: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    // RepoPulled(String),
    ReposCloned,
    ConfigReloaded,
    ConfigReloadedError(String),
}

impl AsRef<Message> for Message {
    fn as_ref(&self) -> &Message {
        self
    }
}

impl AsRef<UpdateRepoMessage> for UpdateRepoMessage {
    fn as_ref(&self) -> &UpdateRepoMessage {
        self
    }
}
