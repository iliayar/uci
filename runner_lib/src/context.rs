use std::{path::PathBuf, sync::Arc};

use common::state::State;
use tokio::sync::Mutex;

use super::config;

use log::*;

pub struct Context<PM: config::ProjectsManager> {
    pub config_path: PathBuf,
    config: Mutex<Arc<config::ServiceConfig>>,
    pub projects_store: config::ProjectsStore<PM>,
}

impl<PM: config::ProjectsManager> Context<PM> {
    pub async fn new(
        projects_store: config::ProjectsStore<PM>,
        config_path: PathBuf,
    ) -> Result<Context<PM>, anyhow::Error> {
        let config = load_config_impl(config_path.clone()).await?;
        Ok(Context {
            config: Mutex::new(Arc::new(config)),
            config_path,
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
        *self.config.lock().await = Arc::new(load_config_impl(self.config_path.clone()).await?);
        Ok(())
    }

    pub async fn init_projects<'a>(&self, state: &State<'a>) -> Result<(), anyhow::Error> {
        let mut state = state.clone();
        let config = self.config.lock().await.clone();
        state.set(config.as_ref());
        self.projects_store.init(&state).await?;
        Ok(())
    }

    pub async fn update_repo<'a>(
        &self,
        state: &State<'a>,
        project_id: &str,
        repo_id: &str,
    ) -> Result<(), anyhow::Error> {
        let mut state = state.clone();
        let config = self.config.lock().await.clone();
        state.set(config.as_ref());
        self.projects_store
            .update_repo(&state, project_id, repo_id)
            .await?;
        Ok(())
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
}

async fn load_config_impl(config_path: PathBuf) -> Result<config::ServiceConfig, anyhow::Error> {
    let mut context = State::default();
    context.set_named("service_config", &config_path);
    let config = config::ServiceConfig::load(&context).await?;

    info!("Loaded config: {:#?}", config);
    Ok(config)
}
