use std::collections::{HashMap, HashSet};

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

pub struct StaticProjects {
    static_projects: HashMap<String, StaticProject>,
}

pub struct StaticProject {
    path: PathBuf,
    repos: super::Repos,
}

#[derive(Debug)]
pub struct MatchedActions {
    pub reload_config: bool,
    pub run_pipelines: HashMap<String, HashSet<String>>,
    pub services: HashMap<String, HashMap<String, super::ServiceAction>>,
    pub reload_projects: HashSet<String>,
}

impl Default for MatchedActions {
    fn default() -> Self {
        Self {
            reload_config: false,
            run_pipelines: Default::default(),
            services: Default::default(),
            reload_projects: Default::default(),
        }
    }
}

impl MatchedActions {
    pub fn is_empty(&self) -> bool {
        !self.reload_config
            && self.reload_projects.is_empty()
            && self.run_pipelines.is_empty()
            && self.services.is_empty()
    }

    pub fn check_allowed<S: AsRef<str> + Clone>(
        &self,
        token: Option<S>,
        config: &super::ServiceConfig,
    ) -> bool {
        if self.reload_config {
            if !config.check_allowed::<_, &str>(token.clone(), None, super::ActionType::Write) {
                return false;
            }
        }

        for project_id in self.reload_projects.iter() {
            if !config.check_allowed::<_, &str>(
                token.clone(),
                Some(project_id),
                super::ActionType::Write,
            ) {
                return false;
            }
        }

        for (project_id, _) in self.run_pipelines.iter() {
            if !config.check_allowed(token.clone(), Some(project_id), super::ActionType::Execute) {
                return false;
            }
        }

        for (project_id, _) in self.services.iter() {
            if !config.check_allowed(token.clone(), Some(project_id), super::ActionType::Execute) {
                return false;
            }
        }

        return true;
    }

    pub fn add_project(
        &mut self,
        project_id: &str,
        super::ProjectMatchedActions {
            reload_config,
            run_pipelines,
            services,
            reload_project,
        }: super::ProjectMatchedActions,
    ) {
        self.reload_config |= reload_config;
        if !run_pipelines.is_empty() {
            self.run_pipelines
                .insert(project_id.to_string(), run_pipelines);
        }
        if !services.is_empty() {
            self.services.insert(project_id.to_string(), services);
        }
        if reload_project {
            self.reload_projects.insert(project_id.to_string());
        }
    }

    pub fn get_project(&self, project_id: &str) -> Option<super::ProjectMatchedActions> {
        let run_pipelines = self
            .run_pipelines
            .get(project_id)
            .cloned()
            .unwrap_or_default();
        let services = self.services.get(project_id).cloned().unwrap_or_default();
        let reload_project = self.reload_projects.contains(project_id);

        let res = super::ProjectMatchedActions {
            reload_config: false,
            run_pipelines,
            services,
            reload_project,
        };

        if res.is_empty() {
            None
        } else {
            Some(res)
        }
    }

    pub fn merge(&mut self, other: MatchedActions) {
        self.reload_config |= other.reload_config;

        for project_id in other.reload_projects.into_iter() {
            self.reload_projects.insert(project_id);
        }

        for (project_id, run_pipelines) in other.run_pipelines.into_iter() {
            if self.run_pipelines.contains_key(&project_id) {
                self.run_pipelines
                    .insert(project_id.clone(), HashSet::new());
            }

            let cur_run_pipelines = self.run_pipelines.get_mut(&project_id).unwrap();
            for pipeline_id in run_pipelines.into_iter() {
                cur_run_pipelines.insert(pipeline_id);
            }
        }

        for (project_id, services) in other.services.into_iter() {
            if self.services.contains_key(&project_id) {
                self.services.insert(project_id.clone(), HashMap::new());
            }
            let cur_services = self.services.get_mut(&project_id).unwrap();
            for (service, action) in services.into_iter() {
                cur_services.insert(service, action);
            }
        }
    }
}

impl Projects {
    // TODO: Load project independed
    pub async fn load<'a>(context: &LoadContext<'a>) -> Result<Projects, LoadConfigError> {
        raw_deprecated::load(context).await
    }

    pub fn get(&self, project: &str) -> Option<&super::Project> {
        self.projects.get(project)
    }

    pub fn list_projects(&self) -> HashSet<String> {
        self.projects.keys().cloned().collect()
    }

    pub async fn get_matched(
        &self,
        event: &super::Event,
    ) -> Result<MatchedActions, super::ExecutionError> {
        let mut matched = MatchedActions::default();
        for (project_id, project) in self.projects.iter() {
            matched.add_project(project_id, project.get_matched_actions(event).await?);
        }
        Ok(matched)
    }

    pub async fn run_matched(
        &self,
        execution_context: &super::ExecutionContext,
        matched: MatchedActions,
    ) -> Result<(), super::ExecutionError> {
        let mut tasks = Vec::new();

        for (project_id, project) in self.projects.iter() {
            debug!("Running matched for project {}", project_id);
            if let Some(project_actions) = matched.get_project(project_id) {
                if execution_context.check_allowed(Some(project_id), super::ActionType::Execute) {
                    warn!(
                        "Not allowed to execute actions on project {}, skiping",
                        project_id
                    );
                    continue;
                }

                tasks.push(project.run_matched_action(execution_context, project_actions));
            }
        }

        futures::future::try_join_all(tasks).await?;
        Ok(())
    }
}

mod raw {
    use std::collections::HashMap;

    use crate::lib::{config, utils};

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Projects {
        projects: HashMap<String, Project>,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Project {
        path: String,
        repos: HashMap<String, config::repos_raw::Repo>,
    }

    const PROJECTS_CONFIG: &str = "projects.yaml";

    impl config::LoadRawSync for Projects {
        type Output = super::StaticProjects;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            Ok(super::StaticProjects {
                static_projects: self.projects.load_raw(context)?,
            })
        }
    }

    impl config::LoadRawSync for Project {
        type Output = super::StaticProject;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            let path = utils::abs_or_rel_to_dir(self.path, context.configs_root()?.clone());

	    let project_id = context.extra("_id")?.to_string();
	    let mut context = context.clone();
	    context.set_project_id(&project_id);

            Ok(super::StaticProject {
                path,
                repos: self.repos.load_raw(&context)?,
            })
        }
    }

    pub async fn load<'a>(
        context: &config::LoadContext<'a>,
    ) -> Result<super::Projects, super::LoadConfigError> {
        let path = context.configs_root()?.join(PROJECTS_CONFIG);
        config::load::<Projects>(path.clone(), context)
            .await
            .map_err(|err| {
                anyhow::anyhow!("Failed to load pipeline from {:?}: {}", path, err).into()
            })
    }
}

mod raw_deprecated {
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
        let path = context.configs_root()?.join(PROJECTS_CONFIG);
        config::load::<Projects>(path.clone(), context)
            .await
            .map_err(|err| {
                anyhow::anyhow!("Failed to load pipeline from {:?}: {}", path, err).into()
            })
    }
}

async fn reset_dir(path: PathBuf) -> Result<(), LoadConfigError> {
    if let Err(err) = tokio::fs::remove_dir_all(path.clone()).await {
        warn!("Cannot remove directory: {}", err);
    }
    tokio::fs::create_dir_all(path.clone()).await?;
    Ok(())
}
