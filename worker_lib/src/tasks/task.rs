use std::{collections::HashMap, path::PathBuf};

use common::state::State;

use log::*;

pub struct TaskContext {
    pub links: HashMap<String, PathBuf>,
}

#[async_trait::async_trait]
pub trait Task {
    async fn run(self, state: &State) -> Result<(), anyhow::Error>;
}

#[async_trait::async_trait]
impl Task for common::Step {
    async fn run(self, state: &State) -> Result<(), anyhow::Error> {
        debug!("Running step with config {:?}", self);

        match self {
            common::Step::RunShell(config) => config.run(state).await,
            common::Step::BuildImage(config) => config.run(state).await,
            common::Step::Request(config) => config.run(state).await,
            common::Step::RunContainer(config) => config.run(state).await,
            common::Step::Parallel(config) => config.run(state).await,
            common::Step::StopContainer(config) => config.run(state).await,
        }
    }
}
