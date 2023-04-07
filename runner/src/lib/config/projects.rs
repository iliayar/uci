use std::collections::HashMap;

use std::path::PathBuf;

use super::{LoadConfigError, LoadContext};

use log::*;

pub const BIND9_DATA_DIR: &str = "__bind9__";
pub const CADDY_DATA_DIR: &str = "__caddy__";

#[derive(Debug)]
pub struct Projects {
    projects: HashMap<String, super::Project>,
}

impl Projects {
    pub async fn load<'a>(context: &LoadContext<'a>) -> Result<Projects, LoadConfigError> {
        raw::load(context).await
    }

    pub fn get(&self, project: &str) -> Option<&super::Project> {
        self.projects.get(project)
    }

    pub async fn make_dns(&self, config: &super::ServiceConfig) -> Result<(), LoadConfigError> {
        let mut builder = super::BindBuilder::default();
        for (project_id, project) in self.projects.iter() {
            info!("Generating bind9 image source for project {}", project_id);
            if let Some(bind) = project.bind.as_ref() {
                builder.add(&bind)?;
            }
        }

        let dns_path = config.data_path.join(BIND9_DATA_DIR);
        reset_dir(dns_path.clone()).await?;
        builder.build(dns_path).await?;

        Ok(())
    }

    pub async fn make_caddy(&self, config: &super::ServiceConfig) -> Result<(), LoadConfigError> {
        let mut builder = super::CaddyBuilder::default();
        for (project_id, project) in self.projects.iter() {
            info!("Generating caddy config for project {}", project_id);
            if let Some(caddy) = project.caddy.as_ref() {
                builder.add(&caddy)?;
            }
        }

        let caddy_path = config.data_path.join(CADDY_DATA_DIR);
        reset_dir(caddy_path.clone()).await?;
        builder.build(caddy_path).await?;

        Ok(())
    }

    pub async fn autorun(
        &self,
        config: &super::ServiceConfig,
        worker_context: Option<worker_lib::context::Context>,
    ) -> Result<(), super::ExecutionError> {
        for (project_id, project) in self.projects.iter() {
            info!("Autorunning service/pipelines in project {}", project_id);
            project.autorun(config, worker_context.clone()).await?;
        }

        Ok(())
    }

    pub async fn run_matched(
        &self,
        config: &super::ServiceConfig,
        worker_context: Option<worker_lib::context::Context>,
        trigger: &super::Trigger,
    ) -> Result<(), super::ExecutionError> {
        let mut tasks = Vec::new();

        for (project_id, project) in self.projects.iter() {
            tasks.push(project.run_matched(config, worker_context.clone(), trigger));
        }

        futures::future::try_join_all(tasks).await?;
        Ok(())
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};
    use tokio::io::AsyncWriteExt;

    use crate::lib::{config, utils};
    use log::*;

    use super::reset_dir;

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Project {
        path: String,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Projects {
        projects: HashMap<String, Project>,
    }

    const PROJECTS_CONFIG: &str = "projects.yaml";

    #[async_trait::async_trait]
    impl config::LoadRaw for Projects {
        type Output = super::Projects;

        async fn load_raw(
            mut self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            let mut projects = HashMap::new();

            let (internal_project_id, internal_project_path) =
                make_internal_project(context).await?;
            self.projects.insert(
                internal_project_id,
                Project {
                    path: internal_project_path.to_string_lossy().to_string(),
                },
            );

            for (id, Project { path }) in self.projects.into_iter() {
                let project_root = utils::abs_or_rel_to_dir(path, context.configs_root()?.clone());
                if !project_root.exists() {
                    error!("Failed to load project at {:?}. Skiping", project_root);
                    continue;
                }

                let mut context = context.clone();
                context.set_project_id(&id);
                context.set_project_root(&project_root);
                projects.insert(id.clone(), config::Project::load(&context).await?);
            }

            Ok(super::Projects { projects })
        }
    }

    async fn make_internal_project<'a>(
        context: &config::LoadContext<'a>,
    ) -> Result<(String, PathBuf), config::LoadConfigError> {
        let project_id = String::from("__internal_project__");
        let project_root = context.config()?.data_dir.join("__internal_project__");
        super::reset_dir(project_root.clone()).await?;

        write_services_config(project_root.clone(), context).await?;
        write_actions_config(project_root.clone(), context).await?;

        let mut context = context.clone();
        context.set_project_id(&project_id);
        context.set_project_root(&project_root);

        Ok((project_id, project_root))
    }

    async fn write_services_config<'a>(
        project_root: PathBuf,
        context: &config::LoadContext<'a>,
    ) -> Result<(), config::LoadConfigError> {
        let mut services =
            tokio::fs::File::create(project_root.join(config::SERVICES_CONFIG)).await?;
        let mut raw_services = Vec::new();

        if let Ok(_) = context.extra("dns") {
            raw_services.push(String::from(
                r#"
  microci-bind9-configured:
    autostart: true
    build:
      path: ${{config.data.path}}/__bind9__
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
        project_root: PathBuf,
        context: &config::LoadContext<'a>,
    ) -> Result<(), config::LoadConfigError> {
        let mut actions =
            tokio::fs::File::create(project_root.join(config::ACTIONS_CONFIG)).await?;
        let mut raw_actions = Vec::new();

        if let Ok(_) = context.extra("dns") {
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

    pub async fn load<'a>(
        context: &config::LoadContext<'a>,
    ) -> Result<super::Projects, super::LoadConfigError> {
        config::load::<Projects>(context.configs_root()?.join(PROJECTS_CONFIG), context).await
    }
}

async fn reset_dir(path: PathBuf) -> Result<(), LoadConfigError> {
    if let Err(err) = tokio::fs::remove_dir_all(path.clone()).await {
        warn!("Cannot remove directory: {}", err);
    }
    tokio::fs::create_dir_all(path.clone()).await?;
    Ok(())
}
