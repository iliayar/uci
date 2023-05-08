use std::collections::HashMap;

use log::*;

#[async_trait::async_trait]
pub trait Integration
where
    Self: Send + Sync,
{
    async fn handle_pipeline_start(&self) -> Result<(), anyhow::Error>;
    async fn handle_pipeline_fail(&self, error: Option<String>) -> Result<(), anyhow::Error>;
    async fn handle_pipeline_done(&self) -> Result<(), anyhow::Error>;

    async fn handle_job_pending(&self, job: &str) -> Result<(), anyhow::Error>;
    async fn handle_job_progress(&self, job: &str, step: usize) -> Result<(), anyhow::Error>;
    async fn handle_job_done(&self, job: &str) -> Result<(), anyhow::Error>;
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

    pub async fn handle_pipeline_start(&self) {
        self.foreach(|integration| async move { integration.handle_pipeline_start().await })
            .await;
    }
    pub async fn handle_pipeline_fail(&self, error: Option<String>) {
        self.foreach(|integration| {
            let error = error.clone();
            async move { integration.handle_pipeline_fail(error).await }
        })
        .await;
    }
    pub async fn handle_pipeline_done(&self) {
        self.foreach(|integration| async move { integration.handle_pipeline_done().await })
            .await
    }

    pub async fn handle_job_pending(&self, job: &str) {
        self.foreach(|integration| async move { integration.handle_job_pending(job).await })
            .await
    }
    pub async fn handle_job_progress(&self, job: &str, step: usize) {
        self.foreach(|integration| async move { integration.handle_job_progress(job, step).await })
            .await
    }
    pub async fn handle_job_done(&self, job: &str) {
        self.foreach(|integration| async move { integration.handle_job_done(job).await })
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
