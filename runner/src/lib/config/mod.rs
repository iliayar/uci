mod repo;
mod service_config;
mod utils;
mod projects;
mod project;
mod actions;
mod pipelines;
mod config;

pub use repo::*;
pub use service_config::*;
pub use projects::*;
pub use project::*;
pub use actions::*;
pub use pipelines::*;
pub use config::*;

#[derive(Debug, thiserror::Error)]
pub enum LoadConfigError {
    #[error("IO Error: {0}")]
    IOError(#[from] tokio::io::Error),

    #[error("Yaml parsing error: {0}")]
    YamlParseError(#[from] serde_yaml::Error),

    // #[error("Git error: {0}")]
    // GitError(#[from] crate::lib::git::GitError),

    // #[error("Request failed: {0}")]
    // RequestError(#[from] reqwest::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    // #[error("IO Error: {0}")]
    // IOError(#[from] tokio::io::Error),

    // #[error("Yaml parsing error: {0}")]
    // YamlParseError(#[from] serde_yaml::Error),

    #[error("Git error: {0}")]
    GitError(#[from] crate::lib::git::GitError),

    #[error("Request failed: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
