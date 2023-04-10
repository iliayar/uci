use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

use anyhow::anyhow;
use log::*;

use crate::lib::filters::CallContext;

#[derive(Debug)]
pub struct Config {
    pub service_config: super::ServiceConfig,
    pub repos: super::Repos,
    pub projects: super::Projects,
}

pub struct ConfigPreload {
    pub service_config: super::ServiceConfig,
    pub repos: super::Repos,
    configs_root: PathBuf,
    env: String,
}

pub enum ActionEvent {
    ConfigReloaded,
    ProjectReloaded {
        project_id: String,
    },
    DirectCall {
        project_id: String,
        trigger_id: String,
    },
    UpdateRepos {
        repos: Vec<String>,
    },
}

pub struct ExecutionContext {
    pub token: Option<String>,
    pub check_permissions: bool,
    pub worker_context: Option<worker_lib::context::Context>,
    pub config: Arc<Config>,
}

impl ExecutionContext {
    pub fn config(&self) -> &Config {
        self.config.as_ref()
    }
    pub fn service_config(&self) -> &super::ServiceConfig {
        &self.config().service_config
    }
    pub fn check_allowed<S: AsRef<str>>(
        &self,
        project_id: Option<S>,
        action: super::ActionType,
    ) -> bool {
        if !self.check_permissions {
            return true;
        }
        self.config()
            .service_config
            .check_allowed(self.token.as_ref(), project_id, action)
    }
}

impl ConfigPreload {
    pub async fn load(self) -> Result<Config, super::LoadConfigError> {
        let mut load_context = super::LoadContext::default();
        load_context.set_configs_root(&self.configs_root);
        load_context.set_env(&self.env);
        load_context.set_config(&self.service_config);
        load_context.set_repos(&self.repos);

        let projects = super::Projects::load(&load_context).await?;

        Ok(Config {
            service_config: self.service_config,
            repos: self.repos,
            projects,
        })
    }

    pub async fn clone_missing_repos(&self) -> Result<(), super::ExecutionError> {
        self.repos.clone_missing_repos(&self.service_config).await
    }

    pub async fn get_missing_repos(&self) -> Result<HashSet<String>, super::ExecutionError> {
        self.repos.get_missing_repos().await
    }
}

impl Config {
    pub async fn preload(
        configs_root: PathBuf,
        env: String,
    ) -> Result<ConfigPreload, super::LoadConfigError> {
        info!("Preloading config");

        let mut load_context = super::LoadContext::default();
        load_context.set_configs_root(&configs_root);
        load_context.set_env(&env);

        let service_config = super::ServiceConfig::load(&load_context).await?;
        load_context.set_config(&service_config);

        let repos = super::Repos::load(&load_context).await?;
        load_context.set_repos(&repos);

        Ok(ConfigPreload {
            service_config,
            repos,
            configs_root,
            env,
        })
    }

    pub async fn get_projects_actions(
        &self,
        event: ActionEvent,
    ) -> Result<super::MatchedActions, super::ExecutionError> {
        let event = match event {
            ActionEvent::DirectCall {
                project_id,
                trigger_id,
            } => super::Event::Call {
                project_id,
                trigger_id,
            },
            ActionEvent::ConfigReloaded => super::Event::ConfigReloaded,
            ActionEvent::ProjectReloaded { project_id } => {
                super::Event::ProjectReloaded { project_id }
            }
            ActionEvent::UpdateRepos { repos } => {
                let diffs = self
                    .repos
                    .pull_all(&self.service_config, Some(repos.into_iter().collect()))
                    .await?;
                super::Event::RepoUpdate { diffs }
            }
        };

        self.projects.get_matched(&event).await
    }

    pub async fn run_project_actions(
        &self,
        execution_context: &ExecutionContext,
        matched: super::MatchedActions,
    ) -> Result<(), super::ExecutionError> {
        info!("Running actions: {:#?}", matched);
        self.projects
            .run_matched(execution_context, matched)
            .await?;

        Ok(())
    }

    pub async fn reload_project<'a>(
        &self,
        configs_root: PathBuf,
        env: &'a str,
    ) -> Result<(), super::LoadConfigError> {
        todo!()
    }
}
