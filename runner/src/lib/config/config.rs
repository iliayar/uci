use std::{collections::HashMap, path::PathBuf};

use anyhow::anyhow;
use log::*;

#[derive(Debug)]
pub struct Config {
    pub service_config: super::ServiceConfig,
    pub repos: super::Repos,
    pub projects: super::Projects,
}

impl Config {
    pub async fn load(configs_root: PathBuf) -> Result<Config, super::LoadConfigError> {
        info!("Loading config");

        let service_config = super::ServiceConfig::load(configs_root.clone()).await?;
        let repos = super::Repos::load(configs_root.clone()).await?;
        let projects = super::Projects::load(configs_root).await?;

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

    pub async fn run_project_action(
        &self,
        project_id: &str,
        action_id: &str,
    ) -> Result<(), super::ExecutionError> {
        let project = self
            .projects
            .get(project_id)
            .ok_or(anyhow!("Should only be called for existing project"))?;
        let action = project
            .actions
            .get(action_id)
            .ok_or(anyhow!("Should only be called for existing action"))?;
        info!("Running action {} on project {}", action_id, project_id);

        let run_pipelines = action
            .get_matched_pipelines(&self.service_config, &self.repos)
            .await?;

        let mut tasks = Vec::new();
        for pipeline_id in run_pipelines.iter() {
            tasks.push(project.run_pipeline(&self.service_config, pipeline_id))
        }
        futures::future::try_join_all(tasks).await?;

        Ok(())
    }

    pub async fn clone_missing_repos(&self) -> Result<(), super::ExecutionError> {
        self.repos.clone_missing_repos(&self.service_config).await
    }
}
