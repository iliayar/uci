use std::{collections::HashMap, path::PathBuf, sync::Arc};

use common::state::State;
use tokio::sync::Mutex;

use crate::git;

use anyhow::anyhow;
use log::*;

#[async_trait::async_trait]
pub trait ProjectsManager
where
    Self: Send,
{
    async fn get_project_info<'a>(
        &mut self,
        state: &State<'a>,
        project_id: &str,
    ) -> Result<ProjectInfo, anyhow::Error>;
    async fn list_projects<'a>(
        &mut self,
        state: &State<'a>,
    ) -> Result<Vec<ProjectInfo>, anyhow::Error>;
}

type Projects = HashMap<String, Arc<super::Project>>;

#[derive(Default, Debug, Clone)]
pub struct ProjectsStore<PM: ProjectsManager> {
    manager: Arc<Mutex<PM>>,
}

impl<PL: ProjectsManager> ProjectsStore<PL> {
    pub async fn with_manager(manager: PL) -> Result<Self, anyhow::Error> {
        Ok(Self {
            manager: Arc::new(Mutex::new(manager)),
        })
    }

    pub async fn list_projects<'a>(
        &self,
        state: &State<'a>,
    ) -> Result<Vec<ProjectInfo>, anyhow::Error> {
        self.manager.lock().await.list_projects(state).await
    }

    pub async fn get_project_info<'a>(
        &self,
        state: &State<'a>,
        project_id: &str,
    ) -> Result<ProjectInfo, anyhow::Error> {
        self.manager
            .lock()
            .await
            .get_project_info(state, project_id)
            .await
    }

    pub async fn load_project<'a>(
        &self,
        state: &State<'a>,
        project_id: &str,
    ) -> Result<super::Project, anyhow::Error> {
        let project_info = self.get_project_info(state, project_id).await?;
        project_info.clone_missing_repos(state).await?;
        project_info.load(state).await
    }

    pub async fn init<'a>(&self, state: &State<'a>) -> Result<(), anyhow::Error> {
        self.reload_internal_project(state).await?;
        Ok(())
    }

    async fn handle_event<'a>(
        &self,
        state: &State<'a>,
        project_id: &str,
        event: &super::Event,
    ) -> Result<(), anyhow::Error> {
        let project = self.load_project(state, project_id).await?;
        project.handle_event(state, event).await
    }

    async fn reload_internal_project<'a>(&self, state: &State<'a>) -> Result<(), anyhow::Error> {
        let mut caddy_builder = super::CaddyBuilder::default();
        let mut bind_builder = super::BindBuilder::default();

        for project_info in self.list_projects(state).await?.into_iter() {
            let project = project_info.load(state).await?;
            if let Some(caddy) = project.caddy.as_ref() {
                caddy_builder.add(caddy)?;
            }

            if let Some(bind) = project.bind.as_ref() {
                bind_builder.add(bind)?;
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
            project
                .handle_event(
                    state,
                    &super::Event::Call {
                        project_id,
                        trigger_id: "__restart__".to_string(),
                    },
                )
                .await?;
        }

        Ok(())
    }

    pub async fn update_repo<'a>(
        &self,
        state: &State<'a>,
        project_id: &str,
        repo_id: &str,
    ) -> Result<(), anyhow::Error> {
        let project_info = self.get_project_info(state, project_id).await?;
        let diffs = project_info.pull_repo(state, repo_id).await?;
        let need_reload_internal_project = !diffs.is_empty();
        self.handle_event(
            state,
            project_id,
            &super::Event::RepoUpdate {
                repo_id: repo_id.to_string(),
                diffs,
            },
        )
        .await?;
        if need_reload_internal_project {
            self.reload_internal_project(state).await?;
        }
        Ok(())
    }

    pub async fn call_trigger<'a>(
        &self,
        state: &State<'a>,
        project_id: &str,
        trigger_id: &str,
    ) -> Result<(), anyhow::Error> {
        self.handle_event(
            state,
            project_id,
            &super::Event::Call {
                project_id: project_id.to_string(),
                trigger_id: trigger_id.to_string(),
            },
        )
        .await?;
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
    pub async fn load<'a>(&self, state: &State<'a>) -> Result<super::Project, anyhow::Error> {
        let mut state = state.clone();
        state.set(self);
        match super::Project::load(&state).await {
            Ok(project) => Ok(project),
            Err(err) => Err(anyhow!("Failed to load project {}: {}", self.id, err)),
        }
    }

    pub fn check_allowed<S: AsRef<str>>(
        &self,
        token: Option<S>,
        action: super::ActionType,
    ) -> bool {
        self.tokens.check_allowed(token, action)
    }

    pub async fn clone_missing_repos<'a>(&self, state: &State<'a>) -> Result<(), anyhow::Error> {
        self.repos.clone_missing_repos(state).await
    }

    pub async fn pull_repo<'a>(
        &self,
        state: &State<'a>,
        repo_id: &str,
    ) -> Result<git::ChangedFiles, anyhow::Error> {
        let changed_files = self.repos.pull_repo(state, repo_id).await?;
        Ok(changed_files)
    }
}

impl From<&ProjectInfo> for common::vars::Vars {
    fn from(val: &ProjectInfo) -> Self {
        let mut vars = common::vars::Vars::default();
        vars.assign("repos", (&val.repos).into()).ok();
        vars.assign("data.path", (&val.data_path).into()).ok();
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
