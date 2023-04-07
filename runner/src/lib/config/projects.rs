use std::collections::HashMap;

use std::path::PathBuf;

use super::{LoadConfigError, LoadContext};

use log::*;

pub const INTERNAL_DATA_DIR: &str = "internal";
pub const BIND9_DATA_DIR: &str = "bind9";
pub const CADDY_DATA_DIR: &str = "caddy";
pub const INTERNAL_PROJECT_DATA_DIR: &str = "internal_project";

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

    use super::{reset_dir, CADDY_DATA_DIR};

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

            let mut caddy_builder = config::CaddyBuilder::default();
            let mut bind_builder = config::BindBuilder::default();

            for (id, Project { path }) in self.projects.into_iter() {
                let project_root = utils::abs_or_rel_to_dir(path, context.configs_root()?.clone());
                if !project_root.exists() {
                    error!("Failed to load project at {:?}. Skiping", project_root);
                    continue;
                }

                let mut context = context.clone();
                context.set_project_id(&id);
                context.set_project_root(&project_root);

                let project = config::Project::load(&context).await?;

                if let Some(caddy) = project.caddy.as_ref() {
                    caddy_builder.add(&caddy)?;
                }

                if let Some(bind) = project.bind.as_ref() {
                    bind_builder.add(&bind)?;
                }

                projects.insert(id.clone(), project);
            }

            let gen_caddy = caddy_builder.build();
            let gen_bind = bind_builder.build();
            let gen_project = config::codegen::project::GenProject {
                caddy: !gen_caddy.is_empty(),
                bind: !gen_bind.is_empty(),
            };

            if !gen_caddy.is_empty() {
                let path = context
                    .config()?
                    .data_dir
                    .join(super::INTERNAL_DATA_DIR)
                    .join(super::CADDY_DATA_DIR);
                super::reset_dir(path.clone()).await?;
                info!("Generating caddy in {:?}", path);
                gen_caddy.gen(path).await?;
            }

            if !gen_bind.is_empty() {
                let path = context
                    .config()?
                    .data_dir
                    .join(super::INTERNAL_DATA_DIR)
                    .join(super::BIND9_DATA_DIR);
                super::reset_dir(path.clone()).await?;
                info!("Generating bind in {:?}", path);
                gen_bind.gen(path).await?;
            }

            if !gen_project.is_empty() {
                let project_id = String::from("__internal_project__");
                let project_root = context
                    .config()?
                    .data_dir
                    .join(super::INTERNAL_DATA_DIR)
                    .join(super::INTERNAL_PROJECT_DATA_DIR);
                super::reset_dir(project_root.clone()).await?;
                info!("Generating internal project in {:?}", project_root);
                gen_project.gen(project_root.clone()).await?;

                let mut context = context.clone();
                context.set_project_id(&project_id);
                context.set_project_root(&project_root);

                let project = config::Project::load(&context).await?;

                projects.insert(project_id, project);
            }

            Ok(super::Projects { projects })
        }
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
