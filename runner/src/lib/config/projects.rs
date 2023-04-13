use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

use tokio::sync::Mutex;

use crate::lib::{context::WsOutput, git};

use anyhow::anyhow;
use log::*;

#[async_trait::async_trait]
pub trait ProjectsManager
where
    Self: Send,
{
    async fn get_project_info<'a>(
        &mut self,
        state: &super::State<'a>,
        project_id: &str,
    ) -> Result<ProjectInfo, anyhow::Error>;
    async fn list_projects<'a>(
        &mut self,
        state: &super::State<'a>,
    ) -> Result<Vec<ProjectInfo>, anyhow::Error>;
}

type Projects = HashMap<String, Arc<super::Project>>;

#[derive(Default, Debug, Clone)]
pub struct ProjectsStore<PM: ProjectsManager> {
    projects: Arc<Mutex<Projects>>,
    manager: Arc<Mutex<PM>>,
}

impl<PL: ProjectsManager> ProjectsStore<PL> {
    pub async fn with_manager(manager: PL) -> Result<Self, anyhow::Error> {
        Ok(Self {
            projects: Arc::new(Mutex::new(HashMap::default())),
            manager: Arc::new(Mutex::new(manager)),
        })
    }

    pub async fn list_projects<'a>(
        &self,
        state: &super::State<'a>,
    ) -> Result<Vec<ProjectInfo>, anyhow::Error> {
        Ok(self.manager.lock().await.list_projects(state).await?)
    }

    pub async fn get_project_info<'a>(
        &self,
        state: &super::State<'a>,
        project_id: &str,
    ) -> Result<ProjectInfo, anyhow::Error> {
        Ok(self
            .manager
            .lock()
            .await
            .get_project_info(state, project_id)
            .await?)
    }

    pub async fn init<'a>(&self, state: &super::State<'a>) -> Result<(), anyhow::Error> {
        let projects = self.load_enabled_projects(state).await?;
        *self.projects.lock().await = projects;
        self.reload_internal_project(state).await?;
        Ok(())
    }

    pub async fn reload_project<'a>(
        &self,
        state: &super::State<'a>,
        project_id: &str,
    ) -> Result<(), anyhow::Error> {
        let project_info = self.get_project_info(state, project_id).await?;
        project_info.clone_missing_repos(state).await?;
        let project = project_info.load(state).await?;
        debug!("Reloaded project {:#?}", project);
        self.projects
            .lock()
            .await
            .insert(project_id.to_string(), Arc::new(project));
        self.reload_internal_project(state).await?;
        Ok(())
    }

    async fn load_enabled_projects<'a>(
        &self,
        state: &super::State<'a>,
    ) -> Result<Projects, anyhow::Error> {
        let mut res = HashMap::new();
        let projects = self.list_projects(state).await?;

        let mut clone_tasks = Vec::new();
        for project_info in projects.iter() {
            clone_tasks.push(project_info.clone_missing_repos(state));
        }
        futures::future::try_join_all(clone_tasks).await?;

        for project_info in projects.iter() {
            if project_info.enabled {
                let project = project_info.load(state).await?;
                debug!("Loaded project: {:#?}", project);
                res.insert(project.id.clone(), Arc::new(project));
            }
        }
        self.reload_internal_project(state).await?;
        Ok(res)
    }

    async fn reload_internal_project<'a>(
        &self,
        state: &super::State<'a>,
    ) -> Result<(), anyhow::Error> {
        let mut caddy_builder = super::CaddyBuilder::default();
        let mut bind_builder = super::BindBuilder::default();

        for (project_id, project) in self.projects.lock().await.iter() {
            if let Some(caddy) = project.caddy.as_ref() {
                caddy_builder.add(&caddy)?;
            }

            if let Some(bind) = project.bind.as_ref() {
                bind_builder.add(&bind)?;
            }
        }

        let gen_caddy = caddy_builder.build();
        let gen_bind = bind_builder.build();
        let gen_project = super::codegen::project::GenProject {
            caddy: !gen_caddy.is_empty(),
            bind: !gen_bind.is_empty(),
        };

        let service_config: &super::ServiceConfig = state.get()?;
        let internal_data_dir = service_config.internal_path.clone();
        if !gen_caddy.is_empty() {
            let path = internal_data_dir.join(super::CADDY_DATA_DIR);
            reset_dir(path.clone()).await?;
            info!("Generating caddy in {:?}", path);
            gen_caddy.gen(path).await?;
        }

        if !gen_bind.is_empty() {
            let path = internal_data_dir.join(super::BIND9_DATA_DIR);
            reset_dir(path.clone()).await?;
            info!("Generating bind in {:?}", path);
            gen_bind.gen(path).await?;
        }

        if !gen_project.is_empty() {
            let project_id = String::from("__internal_project__");
            let project_root = internal_data_dir.join(super::INTERNAL_PROJECT_DATA_DIR);
            reset_dir(project_root.clone()).await?;
            info!("Generating internal project in {:?}", project_root);
            gen_project.gen(project_root.clone()).await?;

            let project_info = ProjectInfo {
                id: project_id.clone(),
                path: project_root,
                enabled: true,
                repos: super::Repos::default(),
                tokens: super::Tokens::default(),
                secrets: super::Secrets::default(),
                data_path: internal_data_dir,
            };

            let project = project_info.load(state).await?;
            debug!("Loaded internal project {:#?}", project);
            self.projects
                .lock()
                .await
                .insert(project_id, Arc::new(project));
        }

        Ok(())
    }

    pub async fn get_matched(
        &self,
        event: &super::Event,
    ) -> Result<super::MatchedActions, anyhow::Error> {
        let mut matched = super::MatchedActions::default();
        for (project_id, project) in self.projects.lock().await.iter() {
            matched.add_project(project_id, project.get_matched_actions(event).await?);
        }
        Ok(matched)
    }

    pub async fn run_matched<'a>(
        &self,
        state: &super::State<'a>,
        matched: super::MatchedActions,
    ) -> Result<(), anyhow::Error> {
        let mut tasks = Vec::new();

        let projects: Vec<Arc<super::Project>> = self
            .projects
            .lock()
            .await
            .iter()
            .map(|(k, v)| v)
            .cloned()
            .collect();

        for project in projects.iter().cloned() {
            debug!("Running matched for project {}", project.id);
            let project_info = self.get_project_info(state, &project.id).await?;
            if let Some(project_actions) = matched.get_project(&project.id) {
                if project_info.check_allowed(state, super::ActionType::Execute) {
                    warn!(
                        "Not allowed to execute actions on project {}, skiping",
                        project.id
                    );
                    continue;
                }

                tasks.push(async move { project.run_matched_action(state, project_actions).await });
            }
        }

        futures::future::try_join_all(tasks).await?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProjectInfo {
    pub id: String,
    pub enabled: bool,
    pub path: PathBuf,
    pub repos: super::Repos,
    pub tokens: super::Tokens,
    pub secrets: super::Secrets,
    pub data_path: PathBuf,
}

impl ProjectInfo {
    pub async fn load<'a>(
        &self,
        state: &super::State<'a>,
    ) -> Result<super::Project, anyhow::Error> {
        let mut state = state.clone();
        state.set(self);
        match super::Project::load(&state).await {
            Ok(project) => Ok(project),
            Err(err) => Err(anyhow!("Failed to load project {}: {}", self.id, err)),
        }
    }

    pub fn check_allowed_token<S: AsRef<str>>(
        &self,
        token: Option<S>,
        action: super::ActionType,
    ) -> bool {
        self.tokens.check_allowed(token, action)
    }

    pub fn check_allowed<'a>(&self, state: &super::State<'a>, action: super::ActionType) -> bool {
        if let Ok(token) = state.get_named::<String, _>("token") {
            self.tokens.check_allowed(Some(token), action)
        } else {
            self.tokens.check_allowed::<String>(None, action)
        }
    }

    pub async fn clone_missing_repos<'a>(
        &self,
        state: &super::State<'a>,
    ) -> Result<(), anyhow::Error> {
        self.repos.clone_missing_repos(state).await
    }

    pub async fn pull_repo<'a>(
        &self,
        state: &super::State<'a>,
        repo_id: &str,
    ) -> Result<git::ChangedFiles, anyhow::Error> {
        self.repos.pull_repo(state, repo_id).await
    }
}

impl Into<common::vars::Vars> for &ProjectInfo {
    fn into(self) -> common::vars::Vars {
        let mut vars = common::vars::Vars::default();
        vars.assign("repos", (&self.repos).into()).ok();
        vars.assign("data.path", (&self.data_path).into()).ok();
        vars
    }
}

async fn reset_dir(path: PathBuf) -> Result<(), anyhow::Error> {
    if let Err(err) = tokio::fs::remove_dir_all(path.clone()).await {
        warn!("Cannot remove directory: {}", err);
    }
    tokio::fs::create_dir_all(path.clone()).await?;
    Ok(())
}
