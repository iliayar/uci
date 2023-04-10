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
pub struct ConfigReloadReponse {
    pub client_id: Option<String>,
    pub pulling_repos: Option<HashSet<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ReloadConfigMessage {
    // RepoPulled(String),
    ReposCloned,
    ConfigReloaded,
    ConfigReloadedError(String),
}
