use std::path::PathBuf;

use tokio::io::AsyncWriteExt;

use crate::lib::config;

pub struct GenProject {
    pub caddy: bool,
    pub bind: bool,
}

impl GenProject {
    pub async fn gen(
        &self,
        path: PathBuf,
    ) -> Result<(), config::LoadConfigError> {
        self.write_services_config(path.clone()).await?;
        self.write_actions_config(path.clone()).await?;
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
	!self.caddy && !self.bind
    }


    async fn write_services_config<'a>(
        &self,
        project_root: PathBuf,
    ) -> Result<(), config::LoadConfigError> {
        let mut services =
            tokio::fs::File::create(project_root.join(config::SERVICES_CONFIG)).await?;
        let mut raw_services = Vec::new();

        if self.bind {
            raw_services.push(String::from(
                r#"
  microci-bind9-configured:
    autostart: true
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

    async fn write_actions_config<'a>(
        &self,
        project_root: PathBuf,
    ) -> Result<(), config::LoadConfigError> {
        let mut actions =
            tokio::fs::File::create(project_root.join(config::ACTIONS_CONFIG)).await?;
        let mut raw_actions = Vec::new();

        if self.bind {
            raw_actions.push(String::from(
                r#"
  __autostart_bind9__:
    conditions:
      - type: on_config_reload
        services:
          microci-bind9-configured: deploy
"#,
            ))
        }

        if raw_actions.is_empty() {
            actions
                .write_all(
                    r#"
actions: {}
"#
                    .as_bytes(),
                )
                .await?;
        } else {
            actions
                .write_all(
                    r#"
actions:
"#
                    .as_bytes(),
                )
                .await?;
            for raw_action in raw_actions.into_iter() {
                actions.write_all(raw_action.as_bytes()).await?;
            }
        }

        Ok(())
    }
}
