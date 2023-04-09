use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use anyhow::anyhow;
use log::*;

#[derive(Debug)]
pub struct Config {
    pub service_config: super::ServiceConfig,
    pub repos: super::Repos,
    pub projects: super::Projects,
}

pub struct ConfigPreload<'a> {
    pub service_config: super::ServiceConfig,
    pub repos: super::Repos,
    configs_root: PathBuf,
    env: &'a str,
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

impl<'a> ConfigPreload<'a> {
    pub async fn load(self) -> Result<Config, super::LoadConfigError> {
        let mut load_context = super::LoadContext::default();
        load_context.set_configs_root(&self.configs_root);
        load_context.set_env(self.env);
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

    pub async fn has_missing_repos(&self) -> Result<bool, super::ExecutionError> {
        self.repos.has_missing_repos().await
    }
}

impl Config {
    pub async fn preload<'a>(
        configs_root: PathBuf,
        env: &'a str,
    ) -> Result<ConfigPreload, super::LoadConfigError> {
        info!("Preloading config");

        let mut load_context = super::LoadContext::default();
        load_context.set_configs_root(&configs_root);
        load_context.set_env(env);

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
        token: Option<String>,
        check_permissions: bool,
        worker_context: Option<worker_lib::context::Context>,
        matched: super::MatchedActions,
    ) -> Result<(), super::ExecutionError> {
        info!("Running actions: {:#?}", matched);
        self.projects
            .run_matched(
                token,
                check_permissions,
                &self.service_config,
                worker_context,
                matched,
            )
            .await?;

        Ok(())
    }
}
