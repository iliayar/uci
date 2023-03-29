use super::context::Context;
use super::tasks::{self, Task};

use common::Pipeline;

use log::*;
use thiserror::Error;

pub struct Executor {
    context: Context,
}

#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("Task #{1} failed: {0}")]
    TaskError(tasks::TaskError, usize),
}

impl Executor {
    pub fn new(context: Context) -> Result<Executor, ExecutorError> {
        Ok(Executor { context })
    }

    pub async fn run(self, config: Pipeline) {
        if let Err(err) = self.run_impl(config).await {
            error!("Executor failed: {}", err);
        }
    }

    pub async fn run_impl(self, config: Pipeline) -> Result<(), ExecutorError> {
        info!("Running execution");

        for (i, step) in config.steps.into_iter().enumerate() {
            step.run(&self.context)
                .await
                .map_err(|e| ExecutorError::TaskError(e, i))?;
        }

        Ok(())
    }
}
