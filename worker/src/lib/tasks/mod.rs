mod docker_build;
mod docker_run;
mod request;
mod run_shell;

pub use docker_build::*;
pub use docker_run::*;
pub use request::*;
pub use run_shell::*;

pub mod error {
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum TaskError {
        #[error("Docker error: {0}")]
        DockerError(#[from] bollard::errors::Error),

        #[error("IO error: {0}")]
        IOError(#[from] std::io::Error),

        #[error("Request error: {0}")]
        RequestError(#[from] reqwest::Error),

        #[error(transparent)]
        Other(#[from] anyhow::Error),
    }
}
