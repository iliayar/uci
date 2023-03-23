use super::docker::Docker;
use super::tasks;
use common::{Config, Step};
use log::*;
use thiserror::Error;

pub struct Executor {
    docker: Docker,
}

#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("Task #{1} failed: {0}")]
    TaskError(tasks::error::TaskError, usize),
}

impl Executor {
    pub fn new(docker: Docker) -> Result<Executor, ExecutorError> {
        Ok(Executor { docker })
    }

    pub async fn run(self, config: Config) {
        if let Err(err) = self.run_impl(config).await {
            error!("Executor failed: {}", err);
        }
    }

    pub async fn run_impl(self, config: Config) -> Result<(), ExecutorError> {
        info!("Running execution");

        for (i, step) in config.steps.into_iter().enumerate() {
            self.run_step(step)
                .await
                .map_err(|e| ExecutorError::TaskError(e, i))?;
        }

        Ok(())
    }

    // TODO: Make step a trait?
    pub async fn run_step(&self, step: Step) -> Result<(), tasks::error::TaskError> {
        match step {
            Step::RunContainer(config) => {
                info!("Running step RunContainer");
                debug!("With config: {:?}", config);

                tasks::docker_run(&self.docker, config).await?;
            }

            Step::BuildImage(config) => {
                info!("Running step BuildImage");
                debug!("With config: {:?}", config);

                tasks::docker_build(&self.docker, config).await?;
            },
	    Step::RunShell(config) => {
                info!("Running step RunShell");
                debug!("With config: {:?}", config);

                tasks::run_shell_command(&self.docker, config).await?;
	    },
	    Step::Request(config) => {
		info!("Running step Request");
		debug!("With config: {:?}", config);

		tasks::run_request(&config).await?;
	    }
        }

        Ok(())
    }
}
