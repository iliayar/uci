use std::{collections::HashMap, path::PathBuf};

use log::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use serde_yaml;
use tokio::{
    fs::{self, File},
    io::{AsyncBufReadExt, BufReader},
};

use crate::lib::utils::try_expand_home;

pub type Repos = HashMap<String, Repo>;
pub type Projects = HashMap<String, Project>;
pub type Actions = HashMap<String, Action>;
pub type Pipelines = HashMap<String, common::Config>;

// TODO: Make IDs type safe
#[derive(Debug)]
pub struct Config {
    pub service_config: ServiceConfig,
    pub repos: Repos,
    pub projects: Projects,
}

#[derive(Debug)]
pub struct Repo {
    pub source: String,
    pub branch: String,
}

#[derive(Debug)]
pub struct Project {
    pub path: PathBuf,
    pub actions: Actions,
    pub pipelines: Pipelines,
}

#[derive(Debug)]
pub struct ServiceConfig {
    pub repos_path: PathBuf,
}

#[derive(Debug)]
pub struct Action {
    update_repos: Vec<String>,
    conditions: Vec<Condition>,
    run_pipelines: Vec<String>,
}

#[derive(Debug)]
pub enum Condition {
    Always,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO Error: {0}")]
    IOError(#[from] tokio::io::Error),

    #[error("Yaml parsing error: {0}")]
    YamlParseError(#[from] serde_yaml::Error),
}

impl Config {
    pub async fn load(configs_root: PathBuf) -> Result<Config, ConfigError> {
        info!("Loading config");

        let service_config = Config::load_service_config(configs_root.join("conf.yaml")).await?;
        let repos = Config::load_repos(configs_root.join("repos.yaml")).await?;
        let projects =
            Config::load_projects(configs_root.clone(), configs_root.join("projects.yaml")).await?;

        Ok(Config {
            service_config,
            repos,
            projects,
        })
    }

    async fn load_service_config(conf_path: PathBuf) -> Result<ServiceConfig, ConfigError> {
        #[derive(Deserialize)]
        struct RawServiceConfig {
            repos_path: String,
        }

        let conf_raw = fs::read_to_string(conf_path).await?;
        let data: RawServiceConfig = serde_yaml::from_str(&conf_raw)?;

        Ok(ServiceConfig {
            repos_path: try_expand_home(data.repos_path),
        })
    }

    async fn load_repos(repos_config: PathBuf) -> Result<HashMap<String, Repo>, ConfigError> {
        #[derive(Deserialize)]
        struct RawRepo {
            id: String,
            source: String,
            branch: Option<String>,
        }

        #[derive(Deserialize)]
        struct RawRepos {
            repos: Vec<RawRepo>,
        }

        let repos_raw = fs::read_to_string(repos_config).await?;
        let data: RawRepos = serde_yaml::from_str(&repos_raw)?;

        let mut repos = Repos::new();
        for RawRepo { id, source, branch } in data.repos.into_iter() {
            repos.insert(
                id,
                Repo {
                    source,
                    branch: branch.unwrap_or(String::from("master")),
                },
            );
        }

        Ok(repos)
    }

    async fn load_projects(
        configs_root: PathBuf,
        projects_config: PathBuf,
    ) -> Result<HashMap<String, Project>, ConfigError> {
        #[derive(Serialize, Deserialize)]
        struct RawProject {
            id: String,
            path: String,
        }

        #[derive(Deserialize)]
        struct RawProjects {
            projects: Vec<RawProject>,
        }

        let project_raw = fs::read_to_string(projects_config).await?;
        let data: RawProjects = serde_yaml::from_str(&project_raw)?;

        let mut projects = Projects::new();
        for RawProject { id, path } in data.projects.into_iter() {
            let path = try_expand_home(path);
            let path = if path.is_absolute() {
                path
            } else {
                configs_root.join(path)
            };

            projects.insert(id, Config::load_project(path).await?);
        }

        Ok(projects)
    }

    async fn load_project(project_path: PathBuf) -> Result<Project, ConfigError> {
        let _project_config_path = project_path.join("project.yaml");
        let actions_path = project_path.join("actions.yaml");
        let pipelines_config_path = project_path.join("pipelines.yaml");

        Ok(Project {
            path: project_path.clone(),
            actions: Config::load_actions(actions_path).await?,
            pipelines: Config::load_pipelines(project_path, pipelines_config_path).await?,
        })
    }

    async fn load_pipelines(
        project_path: PathBuf,
        pipelines_config_path: PathBuf,
    ) -> Result<Pipelines, ConfigError> {
        #[derive(Deserialize)]
        struct RawPipelines {
            pipelines: Vec<RawPipeline>,
        }

        #[derive(Deserialize)]
        struct RawPipeline {
            id: String,
            path: String,
        }

        let pipelines_raw = fs::read_to_string(pipelines_config_path).await?;
        let data: RawPipelines = serde_yaml::from_str(&pipelines_raw)?;

        let mut pipelines = Pipelines::new();
        for RawPipeline { id, path } in data.pipelines.into_iter() {
            let path = try_expand_home(path);
            let path = if path.is_absolute() {
                path
            } else {
                project_path.join(path)
            };

            pipelines.insert(id, Config::load_pipeline_config(path).await?);
        }

        Ok(pipelines)
    }

    async fn load_pipeline_config(path: PathBuf) -> Result<common::Config, ConfigError> {
        todo!()
    }

    async fn load_actions(actions_path: PathBuf) -> Result<Actions, ConfigError> {
        #[derive(Deserialize)]
        struct ActionsRaw {
            actions: Vec<ActionRaw>,
        }

        #[derive(Deserialize)]
        struct ActionRaw {
            id: String,
            update_repos: Vec<String>,
            conditions: Vec<ConditionRaw>,
            run_pipelines: Vec<String>,
        }

        #[derive(Deserialize)]
        enum ConditionTypeRaw {
            #[serde(rename = "always")]
            Always,
        }

        #[derive(Deserialize)]
        struct ConditionRaw {
            #[serde(rename = "type")]
            t: ConditionTypeRaw,
        }

        let actions_raw = fs::read_to_string(actions_path).await?;
        let actions_data: ActionsRaw = serde_yaml::from_str(&actions_raw)?;

        let mut actions = Actions::new();

        for ActionRaw {
            id,
            update_repos,
            conditions,
            run_pipelines,
        } in actions_data.actions.into_iter()
        {
            let conditions = conditions
                .into_iter()
                .map(|ConditionRaw { t }| match t {
                    ConditionTypeRaw::Always => Condition::Always,
                })
                .collect();

            actions.insert(
                id,
                Action {
                    update_repos,
                    conditions,
                    run_pipelines,
                },
            );
        }

        Ok(actions)
    }
}
