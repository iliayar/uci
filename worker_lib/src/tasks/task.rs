use crate::context::Context;

use thiserror::Error;

use log::*;

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("Docker error: {0}")]
    DockerError(#[from] crate::docker::DockerError),

    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[async_trait::async_trait]
pub trait Task {
    async fn run(self, context: &Context) -> Result<(), TaskError>;
}

#[async_trait::async_trait]
impl Task for common::Step {
    async fn run(self, context: &Context) -> Result<(), TaskError> {
        debug!("Running step with config {:?}", self);

        match self {
	    common::Step::RunShell(config) => config.run(context).await,
            _ => todo!(),
        }
    }
}
