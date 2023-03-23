use std::{collections::HashMap, path::PathBuf};

use anyhow::anyhow;
use futures::future::try_join_all;
use log::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use serde_yaml;
use tokio::{
    fs::{self, File},
    io::{AsyncBufReadExt, BufReader},
};

use crate::lib::{git, utils::try_expand_home};

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

impl Project {
    pub async fn run_action(&self, action_id: &str) {}
}

#[derive(Debug)]
pub struct ServiceConfig {
    pub repos_path: PathBuf,
    pub worker_url: String,
}

#[derive(Debug)]
pub struct Action {
    update_repos: Vec<String>,
    cases: Vec<Case>,
}

#[derive(Debug)]
pub struct Case {
    condition: Condition,
    run_pipelines: Vec<String>,
}

#[derive(Debug)]
pub enum Condition {
    Always,
}

impl Condition {
    async fn check_matched(&self, diffs: &HashMap<String, Vec<String>>) -> bool {
        match self {
            Condition::Always => true,
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO Error: {0}")]
    IOError(#[from] tokio::io::Error),

    #[error("Yaml parsing error: {0}")]
    YamlParseError(#[from] serde_yaml::Error),

    #[error("Git error: {0}")]
    GitError(#[from] git::GitError),

    #[error("Request failed: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Config {
    pub async fn has_project_action(&self, project_id: &str, action_id: &str) -> bool {
        self.projects
            .get(project_id)
            .map(|p| p.actions.contains_key(action_id))
            .unwrap_or(false)
    }

    pub async fn run_project_action(
        &self,
        project_id: &str,
        action_id: &str,
    ) -> Result<(), ConfigError> {
        let project = self
            .projects
            .get(project_id)
            .ok_or(anyhow!("Should only be called for existing project"))?;
        let action = project
            .actions
            .get(action_id)
            .ok_or(anyhow!("Should only be called for existing action"))?;
        info!("Running action {} on project {}", action_id, project_id);

        info!("Pulling repos");
        let mut run_pipelines = Vec::new();
        let mut repo_diffs = HashMap::new();
        for repo_id in action.update_repos.iter() {
            let repo = self
                .repos
                .get(repo_id)
                .ok_or(anyhow!("No such repo: {}", repo_id))?;
            let repo_path = self.service_config.repos_path.join(repo_id);
            // TODO: Get rid of this clone?
            let diffs = git::pull_ssh(repo_path, repo.branch.clone()).await?;

            repo_diffs.insert(repo_id.clone(), diffs);
        }

	for (i, case) in action.cases.iter().enumerate() {
	    if case.condition.check_matched(&repo_diffs).await {
		info!("Match condition {}", i);
		run_pipelines.append(&mut case.run_pipelines.clone());
	    }
	}

        let mut tasks = Vec::new();
        for pipeline_id in run_pipelines.iter() {
            let config = project
                .pipelines
                .get(pipeline_id)
                .ok_or(anyhow!("Now such pipeline to run {}", pipeline_id))?;
            tasks.push(self.run_pipeline(pipeline_id, &config));
        }
        try_join_all(tasks).await?;

        Ok(())
    }

    async fn run_pipeline(
        &self,
        pipeline_id: &str,
        config: &common::Config,
    ) -> Result<(), ConfigError> {
        info!("Running pipeline {}", pipeline_id);
        let response = reqwest::Client::new()
            .post(&format!("{}/run", self.service_config.worker_url))
            .json(config)
            .send()
            .await?;

        response.error_for_status()?;

        info!("Pipeline {} started", pipeline_id);

        Ok(())
    }

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

    // TODO: Put all of these in appropriate structs impls

    async fn load_service_config(conf_path: PathBuf) -> Result<ServiceConfig, ConfigError> {
        #[derive(Deserialize)]
        struct RawServiceConfig {
            repos_path: String,
            worker_url: String,
        }

        let conf_raw = fs::read_to_string(conf_path).await?;
        let data: RawServiceConfig = serde_yaml::from_str(&conf_raw)?;

        Ok(ServiceConfig {
            repos_path: try_expand_home(data.repos_path),
            worker_url: data.worker_url,
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
        #[derive(Deserialize)]
        struct StepsRaw {
            steps: Vec<StepRaw>,
        }

        #[derive(Deserialize)]
        struct StepRaw {
            #[serde(rename = "type")]
            t: TypeRaw,
            script: Option<String>,
        }

        #[derive(Deserialize)]
        enum TypeRaw {
            #[serde(rename = "script")]
            Script,
        }

        let pipeline_raw = fs::read_to_string(path).await?;
        let data: StepsRaw = serde_yaml::from_str(&pipeline_raw)?;

        let mut steps = Vec::<common::Step>::new();

        for step_raw in data.steps.into_iter() {
            match step_raw.t {
                TypeRaw::Script => {
                    let config = common::RunShellConfig {
                        script: step_raw
                            .script
                            .ok_or(anyhow!("'script' step requires 'scipt' field"))?,
                        docker_image: None,
                    };
                    steps.push(common::Step::RunShell(config));
                }
            }
        }

        Ok(common::Config { steps })
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
            run_pipelines: Vec<String>,
        }

        let actions_raw = fs::read_to_string(actions_path).await?;
        let actions_data: ActionsRaw = serde_yaml::from_str(&actions_raw)?;

        let mut actions = Actions::new();

        for ActionRaw {
            id,
            update_repos,
            conditions,
        } in actions_data.actions.into_iter()
        {
            let cases = conditions
                .into_iter()
                .map(|ConditionRaw { t, run_pipelines }| {
                    let condition = match t {
                        ConditionTypeRaw::Always => Condition::Always,
                    };

                    Case {
                        condition,
                        run_pipelines,
                    }
                })
                .collect();

            actions.insert(
                id,
                Action {
                    update_repos,
                    cases,
                },
            );
        }

        Ok(actions)
    }
}
