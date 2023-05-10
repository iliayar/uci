use std::collections::HashMap;

use common::state::State;
use log::*;

#[async_trait::async_trait]
pub trait Integration
where
    Self: Send + Sync,
{
    async fn handle_pipeline_start(&self, state: &State) -> Result<(), anyhow::Error>;
    async fn handle_pipeline_fail(
        &self,
        state: &State,
        error: Option<String>,
    ) -> Result<(), anyhow::Error>;
    async fn handle_pipeline_done(&self, state: &State) -> Result<(), anyhow::Error>;

    async fn handle_job_pending(&self, state: &State, job: &str) -> Result<(), anyhow::Error>;
    async fn handle_job_progress(
        &self,
        state: &State,
        job: &str,
        step: usize,
    ) -> Result<(), anyhow::Error>;
    async fn handle_job_done(
        &self,
        state: &State,
        job: &str,
        error: Option<String>,
    ) -> Result<(), anyhow::Error>;
    async fn handle_job_skipped(&self, state: &State, job: &str) -> Result<(), anyhow::Error>;
}

#[cfg(test)]
mod test {
    #[tokio::test]
    #[should_panic]
    #[allow(dead_code)]
    async fn test_dyn() {
        let integraion: Box<dyn super::Integration> = unimplemented!();
    }
}

pub struct Integrations {
    items: Vec<Box<dyn Integration>>,
}

impl Integrations {
    pub fn from_map(configs: HashMap<String, serde_json::Value>) -> Result<Self, anyhow::Error> {
        let mut items = Vec::new();
        for (name, config) in configs.into_iter() {
            match super::get_integration(&name, config) {
                Ok(item) => {
                    items.push(item);
                }
                Err(err) => {
                    error!("Failed to init integration {}: {}", name, err);
                }
            }
        }
        Ok(Self { items })
    }

    pub async fn handle_pipeline_start<'a>(&self, state: &State<'a>) {
        self.foreach(|integration| async move { integration.handle_pipeline_start(state).await })
            .await;
    }
    pub async fn handle_pipeline_fail<'a>(&self, state: &State<'a>, error: Option<String>) {
        self.foreach(|integration| {
            let error = error.clone();
            async move { integration.handle_pipeline_fail(state, error).await }
        })
        .await;
    }
    pub async fn handle_pipeline_done<'a>(&self, state: &State<'a>) {
        self.foreach(|integration| async move { integration.handle_pipeline_done(state).await })
            .await
    }

    pub async fn handle_job_pending<'a>(&self, state: &State<'a>, job: &str) {
        self.foreach(|integration| async move { integration.handle_job_pending(state, job).await })
            .await
    }
    pub async fn handle_job_progress<'a>(&self, state: &State<'a>, job: &str, step: usize) {
        self.foreach(|integration| async move {
            integration.handle_job_progress(state, job, step).await
        })
        .await
    }
    pub async fn handle_job_done<'a>(&self, state: &State<'a>, job: &str, error: Option<String>) {
        self.foreach(|integration| {
            let error = error.clone();
            async move { integration.handle_job_done(state, job, error).await }
        })
        .await
    }
    pub async fn handle_job_skipped<'a>(&self, state: &State<'a>, job: &str) {
        self.foreach(|integration| async move { integration.handle_job_skipped(state, job).await })
            .await
    }

    async fn foreach<'a, F, Fut>(&'a self, f: F)
    where
        F: Fn(&'a dyn Integration) -> Fut,
        Fut: futures::Future<Output = Result<(), anyhow::Error>>,
    {
        let mut tasks = Vec::new();
        for item in self.items.iter() {
            tasks.push(async {
                if let Err(err) = f(item.as_ref()).await {
                    error!("Failed to execute integration: {}", err)
                }
            })
        }
        futures::future::join_all(tasks).await;
    }
}
