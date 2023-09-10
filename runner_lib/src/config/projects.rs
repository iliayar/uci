use std::{collections::HashMap, path::PathBuf, sync::Arc};

use common::state::State;
use dynconf::DynValue;
use tokio::sync::Mutex;

use anyhow::anyhow;
use log::*;

use crate::config;

const INTERNAL_PROJECT_ID: &str = "__internal_project__";
const ADMIN_TOKEN: &str = "admin-token";

pub const BIND9_DATA_DIR: &str = "bind9";
pub const CADDY_DATA_DIR: &str = "caddy";
pub const INTERNAL_PROJECT_DATA_DIR: &str = "internal_project";

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

type Projects = HashMap<String, Arc<config::project::Project>>;

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

    pub async fn run_services_actions<'a>(
        &self,
        state: &State<'a>,
        project_id: &str,
        services: Vec<String>,
        action: config::actions::ServiceAction,
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
        action: config::actions::ServiceAction,
    ) -> Result<(), anyhow::Error> {
        self.run_services_actions(state, project_id, vec![service_id.to_string()], action)
            .await
    }

    async fn handle_event<'a>(
        &self,
        state: &State<'a>,
        project_id: &str,
        event: &config::actions::Event,
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
        let mut caddy_builder = config::caddy::CaddyBuilder::default();
        let mut bind_builder = config::bind::BindBuilder::default();

        for project_info in self.list_projects_raw(state).await?.into_iter() {
            let project = project_info.load(state).await?;

            for caddy in project.caddy.iter() {
                caddy_builder.add(caddy)?;
            }

            for bind in project.bind.iter() {
                bind_builder.add(bind)?;
            }
        }

        let service_config: &config::service_config::ServiceConfig = state.get()?;

        let gen_caddy = caddy_builder.build();
        let gen_bind = bind_builder.build();
        let gen_project = super::codegen::project::GenProject {
            caddy: !gen_caddy.is_empty(),
            bind: !gen_bind.is_empty(),
        };

        let internal_data_dir = service_config.internal_path.clone();
        if !gen_caddy.is_empty() {
            let path = internal_data_dir.join(CADDY_DATA_DIR);
            reset_dir(path.clone()).await?;
            info!("Generating caddy in {:?}", path);
            gen_caddy.gen(path).await?;
        }

        if !gen_bind.is_empty() {
            let path = internal_data_dir.join(BIND9_DATA_DIR);
            reset_dir(path.clone()).await?;
            info!("Generating bind in {:?}", path);
            gen_bind.gen(path).await?;
        }

        if !gen_project.is_empty() {
            let project_root = internal_data_dir.join(INTERNAL_PROJECT_DATA_DIR);
            reset_dir(project_root.clone()).await?;
            info!("Generating internal project in {:?}", project_root);

            let project_config = project_root.join("project.yaml");
            gen_project.gen(project_config.clone()).await?;

            let mut tokens = config::permissions::Tokens::default();
            if let Some(token) = service_config.secrets.get(ADMIN_TOKEN) {
                tokens.add(token, config::permissions::Permissions::superuser());
            }

            let mut dyn_state = config::utils::make_dyn_state(state)?;
            let project = dynconf::util::load::<
                dynconf::util::Lazy<config::project::raw::Project>,
            >(&mut dyn_state, project_config)
            .await?;

            return Ok(Some(ProjectInfo {
                tokens,
                id: INTERNAL_PROJECT_ID.to_string(),
                enabled: true,
                repos: config::repo::Repos::default(),
                secrets: config::secrets::Secrets::default(),
                data_path: internal_data_dir,
                projects: vec![project],
            }));
        }

        Ok(None)
    }

    pub async fn reload_internal_project<'a>(
        &self,
        state: &State<'a>,
    ) -> Result<(), anyhow::Error> {
        if let Some(project_info) = self.build_internal_project(state).await? {
            let mut state = state.clone();
            state.set(&project_info);
            let project = project_info.load(&state).await?;
            debug!("Loaded internal project {:#?}", project);
            let res = project
                .handle_event(
                    &state,
                    &config::actions::Event::Call {
                        project_id: project_info.id.clone(),
                        trigger_id: "__restart__".to_string(),
                    },
                )
                .await;

            if let Err(err) = res {
                error!("Failed to reload internal project: {}", err);
            }
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

        if state.get::<UpdateOnly>().map(|v| v.0).unwrap_or(false) {
            return Ok(());
        }

        let need_reload_internal_project = !diffs.is_empty();
        self.handle_event(
            &state,
            project_id,
            &config::actions::Event::RepoUpdate {
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
            &config::actions::Event::Call {
                project_id: project_id.to_string(),
                trigger_id: trigger_id.to_string(),
            },
        )
        .await?;
        Ok(())
    }
}

pub struct UpdateOnly(bool);

#[derive(Clone, Debug, Default)]
pub struct ProjectInfo {
    pub id: String,
    pub enabled: bool,
    pub projects: Vec<dynconf::util::LoadedLazy<super::project::raw::Project>>,
    pub repos: config::repo::Repos,
    pub tokens: config::permissions::Tokens,
    pub secrets: config::secrets::Secrets,
    pub data_path: PathBuf,
}

impl ProjectInfo {
    pub async fn load<'a>(
        &self,
        state: &State<'a>,
    ) -> Result<config::project::Project, anyhow::Error> {
        self.clone_missing_repos(state).await?;
        let mut state = state.clone();
        state.set(self);

        let mut dyn_state = super::utils::make_dyn_state(&state)?;
        let mut project: Option<config::project::Project> = None;
        for additional_project in self.projects.iter() {
            let additional_project = additional_project.clone().load(&mut dyn_state).await?;

            if let Some(current_project) = project.take() {
                project = Some(current_project.merge(additional_project)?);
            } else {
                project = Some(additional_project);
            }
        }

        project.ok_or_else(|| anyhow!("At least one project config must be specified"))
    }

    pub fn check_allowed<S: AsRef<str>>(
        &self,
        token: Option<S>,
        action: config::permissions::ActionType,
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
    ) -> Result<config::repo::Diff, anyhow::Error> {
        let changed_files = self.repos.update_repo(state, repo_id, artifact).await?;
        Ok(changed_files)
    }
}

async fn reset_dir(path: PathBuf) -> Result<(), anyhow::Error> {
    if let Err(err) = tokio::fs::remove_dir_all(path.clone()).await {
        warn!("Cannot remove directory: {}", err);
    }
    tokio::fs::create_dir_all(path.clone()).await?;
    Ok(())
}

pub use dyn_obj::DynProjectInfo;

mod dyn_obj {
    use std::path::PathBuf;

    use crate::config;

    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize)]
    pub struct DynProjectInfo {
        pub id: String,
        pub secrets: Option<config::secrets::DynSecrets>,
        pub data_path: PathBuf,
        pub repos: Option<config::repo::DynRepos>,
    }

    impl From<&super::ProjectInfo> for DynProjectInfo {
        fn from(project_info: &super::ProjectInfo) -> Self {
            Self {
                id: project_info.id.clone(),
                secrets: Some((&project_info.secrets).into()),
                repos: Some((&project_info.repos).into()),
                data_path: project_info.data_path.clone(),
            }
        }
    }
}
