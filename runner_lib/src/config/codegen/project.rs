use std::path::PathBuf;

use tokio::io::AsyncWriteExt;

use anyhow::Result;

pub struct GenProject {
    pub caddy: bool,
    pub bind: bool,
}

impl GenProject {
    pub async fn gen(&self, project_config: PathBuf) -> Result<()> {
        let mut project_config = tokio::fs::File::create(project_config).await?;
        project_config.write_all(self.gen_impl()?.as_bytes()).await?;
        Ok(())
    }

    pub fn gen_impl(&self) -> Result<String> {
        let mut actions_services = String::new();

        if self.bind {
            actions_services.push_str(
                r#"
        microci-bind9-configured: deploy
"#,
            );
        }

        actions_services = if actions_services.is_empty() {
            "{}".to_string()
        } else {
            actions_services
        };

        let mut run_pipelines = String::new();

        if self.caddy {
            run_pipelines.push_str(
                r#"
        - caddy_reload_pipeline
"#,
            );
        }

        run_pipelines = if run_pipelines.is_empty() {
            "[]".to_string()
        } else {
            run_pipelines
        };

        Ok(format!(
            r#"
docker:
  services:
    microci-bind9-configured:
      build:
        path: ${{config.internal_path}}/bind9
      ports:
        - 53:53/udp
      restart: always
      global: true

pipelines:
  caddy_reload_pipeline:
    jobs:
      run_script:
        do:
          type: script
          script: |
            cd caddy
            caddy reload
    links:
      'caddy': ${{config.internal_path}}/caddy

actions:
  __restart__:
    - on: call
      services: {}
      run_pipelines: {}
"#,
            actions_services, run_pipelines
        ))
    }

    pub fn is_empty(&self) -> bool {
        !self.caddy && !self.bind
    }
}
