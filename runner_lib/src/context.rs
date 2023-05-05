use std::{path::PathBuf, sync::Arc};

use common::state::State;
use tokio::sync::Mutex;

use crate::git;

use super::config;

use log::*;

pub struct Context<PM: config::ProjectsManager> {
    pub config_source: ConfigsSource,
    config: Mutex<Arc<config::ServiceConfig>>,
    pub projects_store: config::ProjectsStore<PM>,
}

pub enum ConfigsSource {
    Explicit {
        config: PathBuf,
    },
    Repo {
        url: Option<String>,
        prefix: String,
        path: PathBuf,
    },
}

impl ConfigsSource {
    pub async fn get_config_path(&self) -> Result<PathBuf, anyhow::Error> {
        match self {
            ConfigsSource::Explicit { config } => Ok(config.clone()),
            ConfigsSource::Repo { url, prefix, path } => {
                let need_pull = if let Some(url) = url.as_ref() {
                    if !git::check_exists(path.clone()).await? {
                        git::clone(url.clone(), path.clone()).await?;
                        false
                    } else {
                        true
                    }
                } else {
                    true
                };

                if need_pull {
                    git::pull(path.clone(), "master".to_string()).await?;
                }

                Ok(path.join(prefix).join("uci.yaml"))
            }
        }
    }
}

impl<PM: config::ProjectsManager> Context<PM> {
    pub async fn new(
        projects_store: config::ProjectsStore<PM>,
        config_source: ConfigsSource,
    ) -> Result<Context<PM>, anyhow::Error> {
        let config = load_config_impl(&config_source).await?;
        Ok(Context {
            config: Mutex::new(Arc::new(config)),
            config_source,
            projects_store,
        })
    }

    pub async fn init<'a>(&self, state: &State<'a>) -> Result<(), anyhow::Error> {
        self.init_projects(state).await?;
        Ok(())
    }

    pub async fn config(&self) -> Arc<config::ServiceConfig> {
        return self.config.lock().await.clone();
    }

    pub async fn reload_config(&self) -> Result<(), anyhow::Error> {
        *self.config.lock().await = Arc::new(load_config_impl(&self.config_source).await?);
        Ok(())
    }

    pub async fn init_projects<'a>(&self, state: &State<'a>) -> Result<(), anyhow::Error> {
        let mut state = state.clone();
        let config = self.config.lock().await.clone();
        state.set(config.as_ref());

        let run_context = common::run_context::RunContext::new();
        state.set(&run_context);

        self.projects_store.init(&state).await?;
        Ok(())
    }

    // FIXME: There is a race. The pipeline might be running
    // when pulling changes. It may cause problems...
    pub async fn update_repo<'a>(
        &self,
        state: &State<'a>,
        project_id: &str,
        repo_id: &str,
        artifact: Option<PathBuf>,
    ) -> Result<(), anyhow::Error> {
        let mut state = state.clone();
        let config = self.config.lock().await.clone();
        state.set(config.as_ref());
        self.projects_store
            .update_repo(&state, project_id, repo_id, artifact)
            .await?;
        Ok(())
    }

    pub async fn run_services_actions<'a>(
        &self,
        state: &State<'a>,
        project_id: &str,
        services: Vec<String>,
        action: config::ServiceAction,
    ) -> Result<(), anyhow::Error> {
        let mut state = state.clone();
        let config = self.config.lock().await.clone();
        state.set(config.as_ref());
        self.projects_store
            .run_services_actions(&state, project_id, services, action)
            .await?;
        Ok(())
    }

    pub async fn run_service_action<'a>(
        &self,
        state: &State<'a>,
        project_id: &str,
        service_id: &str,
        action: config::ServiceAction,
    ) -> Result<(), anyhow::Error> {
        self.run_services_actions(state, project_id, vec![service_id.to_string()], action)
            .await
    }

    pub async fn call_trigger<'a>(
        &self,
        state: &State<'a>,
        project_id: &str,
        trigger_id: &str,
    ) -> Result<(), anyhow::Error> {
        let mut state = state.clone();
        let config = self.config.lock().await.clone();
        state.set(config.as_ref());
        self.projects_store
            .call_trigger(&state, project_id, trigger_id)
            .await?;
        Ok(())
    }

    pub async fn list_projects<'a>(
        &self,
        state: &State<'a>,
    ) -> Result<Vec<config::ProjectInfo>, anyhow::Error> {
        let mut state = state.clone();
        let config = self.config.lock().await.clone();
        state.set(config.as_ref());
        self.projects_store.list_projects(&state).await
    }

    pub async fn get_project_info<'a>(
        &self,
        state: &State<'a>,
        project_id: &str,
    ) -> Result<config::ProjectInfo, anyhow::Error> {
        let mut state = state.clone();
        let config = self.config.lock().await.clone();
        state.set(config.as_ref());
        self.projects_store
            .get_project_info(&state, project_id)
            .await
    }

    pub async fn get_project<'a>(
        &self,
        state: &State<'a>,
        project_id: &str,
    ) -> Result<config::Project, anyhow::Error> {
        let mut state = state.clone();
        let config = self.config.lock().await.clone();
        state.set(config.as_ref());
        let project_info = self
            .projects_store
            .get_project_info(&state, project_id)
            .await?;
        project_info.load(&state).await
    }
}

async fn load_config_impl(
    config_source: &ConfigsSource,
) -> Result<config::ServiceConfig, anyhow::Error> {
    let config_path = config_source.get_config_path().await?;

    let mut context = State::default();
    context.set_named("service_config", &config_path);

    let config = config::ServiceConfig::load(&context).await?;

    info!("Loaded config: {:#?}", config);
    Ok(config)
}
