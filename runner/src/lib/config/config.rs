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

pub enum ActionTrigger {
    ConfigReloaded,
    DirectCall {
        project_id: String,
        action_id: String,
    },
}

impl Config {
    pub async fn load(configs_root: PathBuf, env: &str) -> Result<Config, super::LoadConfigError> {
        info!("Loading config");

        let mut load_context = super::LoadContext::default();
        load_context.set_configs_root(&configs_root);
        load_context.set_env(env);

        let service_config = super::ServiceConfig::load(&load_context).await?;
        load_context.set_config(&service_config);

        let repos = super::Repos::load(&load_context).await?;
        load_context.set_repos(&repos);

        let projects = super::Projects::load(&load_context).await?;

        Ok(Config {
            service_config,
            repos,
            projects,
        })
    }

    pub async fn has_project_action(&self, project_id: &str, action_id: &str) -> bool {
        self.projects
            .get(project_id)
            .map(|p| p.actions.get(action_id).is_some())
            .unwrap_or(false)
    }

    pub async fn run_project_actions(
        &self,
        worker_context: Option<worker_lib::context::Context>,
        trigger: ActionTrigger,
    ) -> Result<(), super::ExecutionError> {
        let trigger = match trigger {
            ActionTrigger::DirectCall {
                project_id,
                action_id,
            } => {
                let project = self
                    .projects
                    .get(&project_id)
                    .ok_or(anyhow!("Should only be called for existing project"))?;
                let action = project
                    .actions
                    .get(&action_id)
                    .ok_or(anyhow!("Should only be called for existing action"))?;
                info!("Running action {} on project {}", action_id, project_id);

                let diffs = action.get_diffs(&self.service_config, &self.repos).await?;
                super::Trigger::RepoUpdate(diffs)
            }
            ActionTrigger::ConfigReloaded => super::Trigger::ConfigReloaded,
        };

        self.projects
            .run_matched(&self.service_config, worker_context, &trigger)
            .await?;

        Ok(())
    }

    pub async fn clone_missing_repos(&self) -> Result<(), super::ExecutionError> {
        self.repos.clone_missing_repos(&self.service_config).await
    }
}
