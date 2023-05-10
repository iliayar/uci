use std::{collections::HashMap, path::PathBuf, sync::Arc};

use common::state::State;
use tokio::sync::Mutex;

use anyhow::anyhow;
use log::*;

const INTERNAL_PROJECT_ID: &str = "__internal_project__";
const ADMIN_TOKEN: &str = "admin-token";

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

#[cfg(test)]
mod test {
    #[tokio::test]
    #[should_panic]
    #[allow(unreachable_code)]
    async fn test_project_manager_dyn() {
        let pm: Box<dyn super::ProjectsManager> = unimplemented!();
        let state: super::State = super::State::default();
        pm.get_project_info(&state, "test").await.ok();
    }
}

type Projects = HashMap<String, Arc<super::Project>>;

#[derive(Clone)]
pub struct ProjectsStore {
    manager: Arc<Mutex<Box<dyn ProjectsManager>>>,
}

impl ProjectsStore {
    pub async fn with_manager<PM: ProjectsManager + 'static>(
        manager: PM,
    ) -> Result<Self, anyhow::Error> {
        let manager = Box::new(manager);
        Ok(Self {
            manager: Arc::new(Mutex::new(manager)),
        })
    }

    pub async fn list_projects<'a>(
        &self,
        state: &State<'a>,
    ) -> Result<Vec<ProjectInfo>, anyhow::Error> {
        let mut res = self.manager.lock().await.list_projects(state).await?;

        if let Some(internal_project_info) = self.build_internal_project(state).await? {
            res.push(internal_project_info);
        }

        Ok(res)
    }

    async fn list_projects_raw<'a>(
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
        if project_id == INTERNAL_PROJECT_ID {
            if let Some(internal_project_info) = self.build_internal_project(state).await? {
                return Ok(internal_project_info);
            }
        }

        self.manager
            .lock()
            .await
            .get_project_info(state, project_id)
            .await
    }

    pub async fn init<'a>(&self, state: &State<'a>) -> Result<(), anyhow::Error> {
        self.reload_internal_project(state).await?;
        Ok(())
    }

    pub async fn run_services_actions<'a>(
        &self,
        state: &State<'a>,
        project_id: &str,
        services: Vec<String>,
        action: super::ServiceAction,
    ) -> Result<(), anyhow::Error> {
        let project_info = self.get_project_info(state, project_id).await?;
        let mut state = state.clone();
        state.set(&project_info);
        let project = project_info.load(&state).await?;
        let services = services.into_iter().map(|s| (s, action.clone())).collect();
        project.run_service_actions(&state, services).await
    }

    pub async fn run_service_action<'a>(
        &self,
        state: &State<'a>,
        project_id: &str,
        service_id: &str,
        action: super::ServiceAction,
    ) -> Result<(), anyhow::Error> {
        self.run_services_actions(state, project_id, vec![service_id.to_string()], action)
            .await
    }

    async fn handle_event<'a>(
        &self,
        state: &State<'a>,
        project_id: &str,
        event: &super::Event,
    ) -> Result<(), anyhow::Error> {
        let project_info = self.get_project_info(state, project_id).await?;
        let mut state = state.clone();
        state.set(&project_info);
        let project = project_info.load(&state).await?;
        project.handle_event(&state, event).await
    }

    async fn build_internal_project<'a>(
        &self,
        state: &State<'a>,
    ) -> Result<Option<ProjectInfo>, anyhow::Error> {
        let mut caddy_builder = super::CaddyBuilder::default();
        let mut bind_builder = super::BindBuilder::default();

        for project_info in self.list_projects_raw(state).await?.into_iter() {
            let project = project_info.load(state).await?;
            if let Some(caddy) = project.caddy.as_ref() {
                caddy_builder.add(caddy)?;
            }

            if let Some(bind) = project.bind.as_ref() {
                bind_builder.add(bind)?;
            }
        }

        let service_config: &super::ServiceConfig = state.get()?;

        let gen_caddy = caddy_builder.build();
        let gen_bind = bind_builder.build();
        let gen_project = super::codegen::project::GenProject {
            caddy: !gen_caddy.is_empty(),
            bind: !gen_bind.is_empty(),
        };

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
            let project_root = internal_data_dir.join(super::INTERNAL_PROJECT_DATA_DIR);
            reset_dir(project_root.clone()).await?;
            info!("Generating internal project in {:?}", project_root);
            gen_project.gen(project_root.clone()).await?;

            let mut tokens = super::Tokens::default();
            if let Some(token) = service_config.secrets.get(ADMIN_TOKEN) {
                tokens.add(token, super::Permissions::superuser());
            }

            return Ok(Some(ProjectInfo {
                tokens,
                id: INTERNAL_PROJECT_ID.to_string(),
                path: project_root,
                enabled: true,
                repos: super::Repos::default(),
                secrets: super::Secrets::default(),
                data_path: internal_data_dir,
            }));
        }

        Ok(None)
    }

    async fn reload_internal_project<'a>(&self, state: &State<'a>) -> Result<(), anyhow::Error> {
        if let Some(project_info) = self.build_internal_project(state).await? {
            let mut state = state.clone();
            state.set(&project_info);
            let project = project_info.load(&state).await?;
            debug!("Loaded internal project {:#?}", project);
            project
                .handle_event(
                    &state,
                    &super::Event::Call {
                        project_id: project_info.id.clone(),
                        trigger_id: "__restart__".to_string(),
                    },
                )
                .await?;
        }

        Ok(())
    }

    pub async fn update_repo<'a>(
        &self,
        init_state: &State<'a>,
        project_id: &str,
        repo_id: &str,
        artifact: Option<PathBuf>,
    ) -> Result<(), anyhow::Error> {
        let project_info = self.get_project_info(init_state, project_id).await?;
        let mut state = init_state.clone();
        state.set(&project_info);
        let diffs = project_info.update_repo(&state, repo_id, artifact).await?;

        if state.get_named("update_only").cloned().unwrap_or(false) {
            return Ok(());
        }

        let need_reload_internal_project = !diffs.is_empty();
        self.handle_event(
            &state,
            project_id,
            &super::Event::RepoUpdate {
                repo_id: repo_id.to_string(),
                diffs,
            },
        )
        .await?;
        if need_reload_internal_project {
            self.reload_internal_project(init_state).await?;
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
        self.clone_missing_repos(state).await?;
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

    pub async fn update_repo<'a>(
        &self,
        state: &State<'a>,
        repo_id: &str,
        artifact: Option<PathBuf>,
    ) -> Result<super::Diff, anyhow::Error> {
        let changed_files = self.repos.update_repo(state, repo_id, artifact).await?;
        Ok(changed_files)
    }
}

impl From<&ProjectInfo> for common::vars::Value {
    fn from(val: &ProjectInfo) -> Self {
        let mut vars = common::vars::Value::default();
        vars.assign("repos", (&val.repos).into()).ok();
        vars.assign("data.path", (&val.data_path).into()).ok();
        vars.assign("secrets", (&val.secrets).into()).ok();
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
