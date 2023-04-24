use crate::docker::{self, Docker};

use anyhow::anyhow;
use common::{run_context::RunContext, state::State};

use futures::StreamExt;

#[async_trait::async_trait]
impl super::Task for common::ServiceLogsConfig {
    async fn run(self, state: &State) -> Result<(), anyhow::Error> {
        let docker: &Docker = state.get()?;
        let run_context: &RunContext = state.get()?;

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

        let mut params = docker::LogsParamsBuilder::default();
        params
            .container(self.container)
            .follow(self.follow)
            .tail(self.tail);

        let mut logs = Box::pin(
            docker.logs(
                params
                    .build()
                    .map_err(|e| anyhow!("Invalid stop container params: {}", e))?,
            ),
        );

        loop {
            if !run_context.has_clients().await {
                break;
            }

            // If there is a log, then process it
            // If clock ticks then check for clients
            #[rustfmt::skip]
            let log = tokio::select! {
                log = logs.next() => log,
                _ = interval.tick() => {
                    run_context.heartbeat().await;
                    continue;
                }
            };

            let log = if let Some(log) = log {
                log
            } else {
                break;
            };
            match log {
                Ok(log) => {
                    run_context.send(log).await;
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }

        Ok(())
    }
}
