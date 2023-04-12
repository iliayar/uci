use std::{collections::HashMap, path::PathBuf, sync::Arc};

use tokio::sync::Mutex;

use crate::lib::context::WsOutput;

use log::*;

#[async_trait::async_trait]
pub trait ProjectsManager
where
    Self: Send,
{
    async fn get_project_info<'a>(
        &mut self,
        state: &super::State<'a>,
        project_id: String,
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
        project_id: String,
    ) -> Result<ProjectInfo, super::ExecutionError> {
        Ok(self
            .manager
            .lock()
            .await
            .get_project_info(state, project_id)
            .await?)
    }

    pub async fn reload_projects<'a>(&self, state: &super::State<'a>) -> Result<(), anyhow::Error> {
        let projects = self.load_enabled_projects(state).await?;
        *self.projects.lock().await = projects;
        self.add_internal_project(state).await?;
        Ok(())
    }

    pub async fn reload_project<'a>(
        &self,
        state: &super::State<'a>,
        project_id: String,
    ) -> Result<(), anyhow::Error> {
        let project = self.load_project(state, project_id.clone()).await?;
        self.projects
            .lock()
            .await
            .insert(project_id, Arc::new(project));
        self.add_internal_project(state).await?;
        Ok(())
    }

    async fn reload_internal_project<'a>(
        &self,
        state: &super::State<'a>,
    ) -> Result<(), anyhow::Error> {
        todo!()
    }

    async fn load_project<'a>(
        &self,
        state: &super::State<'a>,
        project_id: String,
    ) -> Result<super::Project, anyhow::Error> {
        let project_info = self
            .manager
            .lock()
            .await
            .get_project_info(state, project_id)
            .await?;
        Ok(project_info.load(state).await?)
    }

    async fn load_enabled_projects<'a>(
        &self,
        state: &super::State<'a>,
    ) -> Result<Projects, anyhow::Error> {
        let mut res = HashMap::new();
        for project in self.list_projects(state).await? {
            if project.enabled {
                let project = self.load_project(state, project.id).await?;
                debug!("Loaded project: {:#?}", project);
                res.insert(project.id.clone(), Arc::new(project));
            }
        }
        self.add_internal_project(state).await?;
        Ok(res)
    }

    async fn add_internal_project<'a>(
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
        let data_dir = service_config.data_dir.clone();
        if !gen_caddy.is_empty() {
            let path = data_dir
                .join(super::INTERNAL_DATA_DIR)
                .join(super::CADDY_DATA_DIR);
            reset_dir(path.clone()).await?;
            info!("Generating caddy in {:?}", path);
            gen_caddy.gen(path).await?;
        }

        if !gen_bind.is_empty() {
            let path = data_dir
                .join(super::INTERNAL_DATA_DIR)
                .join(super::BIND9_DATA_DIR);
            reset_dir(path.clone()).await?;
            info!("Generating bind in {:?}", path);
            gen_bind.gen(path).await?;
        }

        if !gen_project.is_empty() {
            let project_id = String::from("__internal_project__");
            let project_root = data_dir
                .join(super::INTERNAL_DATA_DIR)
                .join(super::INTERNAL_PROJECT_DATA_DIR);
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
                data_path: data_dir.join(super::INTERNAL_DATA_DIR),
            };

            self.projects
                .lock()
                .await
                .insert(project_id, Arc::new(project_info.load(state).await?));
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
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
    ) -> Result<super::Project, super::LoadConfigError> {
        let mut state = state.clone();
        state.set(self);
        super::Project::load(&state).await
    }

    pub fn check_allowed<S: AsRef<str>>(
        &self,
        token: Option<S>,
        action: super::ActionType,
    ) -> bool {
        self.tokens.check_allowed(token, action)
    }
}

async fn reset_dir(path: PathBuf) -> Result<(), anyhow::Error> {
    if let Err(err) = tokio::fs::remove_dir_all(path.clone()).await {
        warn!("Cannot remove directory: {}", err);
    }
    tokio::fs::create_dir_all(path.clone()).await?;
    Ok(())
}
