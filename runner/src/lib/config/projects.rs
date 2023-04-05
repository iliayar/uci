use std::collections::HashMap;

use std::path::PathBuf;

use super::{LoadConfigError, LoadContext};

use log::*;

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
}

mod raw {
    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};

    use crate::lib::{config, utils};

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
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            let mut projects = HashMap::new();

            for (id, Project { path }) in self.projects.into_iter() {
                let project_root = utils::abs_or_rel_to_dir(path, context.configs_root()?.clone());
                let mut context = context.clone();
                context.set_project_id(&id);
                context.set_project_root(&project_root);
                projects.insert(id.clone(), config::Project::load(&context).await?);
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
