use std::path::PathBuf;

use tokio::io::AsyncWriteExt;

use crate::imp::config;

pub struct GenProject {
    pub caddy: bool,
    pub bind: bool,
}

impl GenProject {
    pub async fn gen(&self, path: PathBuf) -> Result<(), anyhow::Error> {
        self.write_services_config(path.clone()).await?;
        self.write_actions_config(path.clone()).await?;
        self.write_pipelines(path.clone()).await?;
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        !self.caddy && !self.bind
    }

    async fn write_services_config<'a>(&self, project_root: PathBuf) -> Result<(), anyhow::Error> {
        let mut services =
            tokio::fs::File::create(project_root.join(config::SERVICES_CONFIG)).await?;
        let mut raw_services = Vec::new();

        if self.bind {
            raw_services.push(String::from(
                r#"
  microci-bind9-configured:
    build:
      path: ${config.internal.path}/bind9
    ports:
      - 3053:53/udp
    restart: always
    global: true
"#,
            ))
        }

        if raw_services.is_empty() {
            services
                .write_all(
                    r#"
services: {}
"#
                    .as_bytes(),
                )
                .await?;
        } else {
            services
                .write_all(
                    r#"
services:
"#
                    .as_bytes(),
                )
                .await?;
            for raw_service in raw_services.into_iter() {
                services.write_all(raw_service.as_bytes()).await?;
            }
        }
        Ok(())
    }

    async fn write_actions_config<'a>(&self, project_root: PathBuf) -> Result<(), anyhow::Error> {
        let mut actions =
            tokio::fs::File::create(project_root.join(config::ACTIONS_CONFIG)).await?;
        let mut raw_run_pipelines = Vec::new();
        let mut raw_services = Vec::new();
        actions
            .write_all(
                r#"
actions:
  __restart__:
    - on: call
"#
                .as_bytes(),
            )
            .await?;

        if self.bind {
            raw_services.push(String::from("microci-bind9-configured: deploy"))
        }

        if self.caddy {
            raw_run_pipelines.push(String::from("caddy_reload_pipeline"));
        }

        if !raw_services.is_empty() {
            actions
                .write_all(
                    r#"
      services:
"#
                    .as_bytes(),
                )
                .await?;
            for service in raw_services.into_iter() {
                actions
                    .write_all(
                        format!(
                            r#"
        {}
"#,
                            service
                        )
                        .as_bytes(),
                    )
                    .await?;
            }
        }

        if !raw_run_pipelines.is_empty() {
            actions
                .write_all(
                    r#"
      run_pipelines:
"#
                    .as_bytes(),
                )
                .await?;
            for pipeline in raw_run_pipelines.into_iter() {
                actions
                    .write_all(
                        format!(
                            r#"
        - {}
"#,
                            pipeline
                        )
                        .as_bytes(),
                    )
                    .await?;
            }
        }

        Ok(())
    }

    async fn write_pipelines<'a>(&self, project_root: PathBuf) -> Result<(), anyhow::Error> {
        let mut pipelines =
            tokio::fs::File::create(project_root.join(config::PIPELINES_CONFIG)).await?;
        let mut raw_pipelines = Vec::new();

        if self.caddy {
            self.write_caddy_reload_pipeline(project_root.clone())
                .await?;
            raw_pipelines.push(String::from(
                r#"
  caddy_reload_pipeline:
    path: caddy_reload_pipeline.yaml
 
"#,
            ));
        }

        if raw_pipelines.is_empty() {
            pipelines
                .write_all(
                    r#"
pipelines: {}
"#
                    .as_bytes(),
                )
                .await?;
        } else {
            pipelines
                .write_all(
                    r#"
pipelines:
"#
                    .as_bytes(),
                )
                .await?;
            for raw_pipeline in raw_pipelines.into_iter() {
                pipelines.write_all(raw_pipeline.as_bytes()).await?;
            }
        }

        Ok(())
    }

    async fn write_caddy_reload_pipeline<'a>(
        &self,
        project_root: PathBuf,
    ) -> Result<(), anyhow::Error> {
        let mut pipeline =
            tokio::fs::File::create(project_root.join("caddy_reload_pipeline.yaml")).await?;

        pipeline
            .write_all(
                r#"
jobs:
  run_script:
    steps:
      - script: |
          cd caddy
          caddy reload
links:
  'caddy': ${config.internal.path}/caddy
"#
                .as_bytes(),
            )
            .await?;

        Ok(())
    }
}
