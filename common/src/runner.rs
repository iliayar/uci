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
    pub client_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    // RepoPulled(String),
    ReposCloned,
    ConfigReloaded,
    ConfigReloadedError(String),
}
