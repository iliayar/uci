use std::{collections::HashMap, path::PathBuf, sync::Arc};

use tokio::sync::Mutex;

use crate::lib::context::WsOutput;

pub struct ExecutionContext {
    pub check_permissions: bool,
    pub worker_context: Option<worker_lib::context::Context>,
    pub worker_url: Option<String>,
    pub permission: super::Permissions,
    pub ws_ouput: WsOutput,
}

impl ExecutionContext {
    pub fn check_allowed<S: AsRef<str>>(&self, action: super::ActionType) -> bool {
        if !self.check_permissions {
            return true;
        }
        self.permission.check_allowed(action)
    }
}

#[async_trait::async_trait]
pub trait ProjectsManager
where
    Self: Send,
{
    async fn get_project_info(&mut self, project_id: String) -> Result<ProjectInfo, anyhow::Error>;
    async fn list_projects(&mut self) -> Result<Vec<ProjectInfo>, anyhow::Error>;
    async fn load_project(&mut self, project_id: String) -> Result<super::Project, anyhow::Error>;

    async fn reload_projects(&mut self) -> Result<(), anyhow::Error>;

    async fn load_enabled_projects(&mut self) -> Result<Projects, anyhow::Error> {
        let mut res = HashMap::new();
        for project in self.list_projects().await? {
            if project.enabled {
                res.insert(project.id.clone(), Arc::new(self.load_project(project.id).await?));
            }
        }
        Ok(res)
    }
}

type Projects = HashMap<String, Arc<super::Project>>;

#[derive(Default, Debug, Clone)]
pub struct ProjectsStore<PM: ProjectsManager> {
    projects: Arc<Mutex<Projects>>,
    manager: Arc<Mutex<PM>>,
}

impl<PL: ProjectsManager> ProjectsStore<PL> {
    pub async fn with_manager(mut manager: PL) -> Result<Self, anyhow::Error> {
        let projects = Arc::new(Mutex::new(manager.load_enabled_projects().await?));
        Ok(Self {
            projects,
            manager: Arc::new(Mutex::new(manager)),
        })
    }

    pub async fn get_projects_info(&self) -> Result<Vec<ProjectInfo>, anyhow::Error> {
        Ok(self.manager.lock().await.list_projects().await?)
    }

    pub async fn get_project_info(
        &self,
        project_id: String,
    ) -> Result<ProjectInfo, super::ExecutionError> {
        Ok(self
            .manager
            .lock()
            .await
            .get_project_info(project_id)
            .await?)
    }

    pub async fn reload_projects(&self) -> Result<(), anyhow::Error> {
        *self.projects.lock().await = self.manager.lock().await.load_enabled_projects().await?;
        Ok(())
    }

    pub async fn reload_project(&self, project_id: String) -> Result<(), anyhow::Error> {
	let project = self.manager.lock().await.load_project(project_id.clone()).await?;
	self.projects.lock().await.insert(project_id, Arc::new(project));
	Ok(())
    }
}

pub struct ProjectInfo {
    pub id: String,
    pub enabled: bool,
    pub repos: super::Repos,
    pub tokens: super::Tokens,
    pub secrets: super::Secrets,
}
