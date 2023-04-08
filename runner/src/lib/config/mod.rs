mod actions;
mod bind;
mod caddy;
mod codegen;
mod config;
mod load;
mod pipelines;
mod project;
mod projects;
mod repo;
mod service_config;
mod services;
mod utils;

pub use actions::*;
pub use bind::*;
pub use caddy::*;
pub use config::*;
pub use load::*;
pub use pipelines::*;
pub use project::*;
pub use projects::*;
pub use repo::*;
pub use service_config::*;
pub use services::*;

#[derive(Debug, thiserror::Error)]
pub enum LoadConfigError {
    #[error("IO Error: {0}")]
    IOError(#[from] tokio::io::Error),

    #[error("Yaml parsing error: {0}")]
    YamlParseError(#[from] serde_yaml::Error),

    #[error("Failed to substitute vars: {0}")]
    SubstitutionError(#[from] common::vars::SubstitutionError),

    #[error("Codegen error: {0}")]
    CodegenError(#[from] codegen::CodegenError),

    #[error("Invalid regex: {0}")]
    InvalidRegex(#[from] regex::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("Git error: {0}")]
    GitError(#[from] crate::lib::git::GitError),

    #[error("Request failed: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("Pipeline failed: {0}")]
    PipelineError(#[from] worker_lib::executor::ExecutorError),

    #[error("IO Error: {0}")]
    IOError(#[from] tokio::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
