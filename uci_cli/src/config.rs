use std::path::PathBuf;

use clap::CommandFactory;

use termion::{color, style};

#[derive(Debug, Default)]
pub struct Config {
    pub runner_url: Option<String>,
    pub ws_runner_url: Option<String>,
    pub token: Option<String>,
    pub default_project: Option<String>,
    pub project_arg: Option<Option<String>>,
}

impl runner_client::RunnerClientConfig for Config {
    fn ws_runner_url(&self) -> Option<&str> {
        self.ws_runner_url.as_ref().map(|u| u.as_str())
    }

    fn runner_url(&self) -> Option<&str> {
        self.runner_url.as_ref().map(|u| u.as_str())
    }

    fn token(&self) -> Option<&str> {
        self.runner_url.as_ref().map(|u| u.as_str())
    }
}

impl Config {
    pub async fn load(
        path: PathBuf,
        env: String,
        project: Option<String>,
        prompt_project: bool,
    ) -> Result<Config, anyhow::Error> {
        raw::load(path, env, project, prompt_project).await
    }

    pub async fn try_get_project(&self) -> Option<String> {
        if let Some(project_id) = self.project_arg.as_ref() {
            if let Some(project_id) = project_id.as_ref() {
                Some(project_id.to_string())
            } else {
                Some(
                    crate::prompts::promp_project(&self)
                        .await
                        .expect("Project wasn't selected"),
                )
            }
        } else if let Some(project_id) = self.default_project.as_ref() {
            eprintln!(
                "{}Using default project {}{}{}",
                color::Fg(color::Yellow),
                style::Bold,
                project_id,
                style::Reset
            );
            Some(project_id.to_string())
        } else {
            None
        }
    }

    pub async fn get_project(&self) -> String {
        if let Some(project_id) = self.try_get_project().await {
            project_id
        } else {
            eprintln!(
                "{}No project specified either in args nor in config{}",
                color::Fg(color::Red),
                style::Reset
            );
            let mut cmd = super::cli::Cli::command();
            cmd.print_help().ok();
            std::process::exit(1);
        }
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use log::*;

    type ConfigEnvs = HashMap<String, Config>;

    #[derive(Deserialize, Serialize, Clone, Default)]
    struct Config {
        pub ws_runner_url: Option<String>,
        pub runner_url: Option<String>,
        pub token: Option<String>,
        pub default_project: Option<String>,
        pub prompt_project: Option<bool>,
    }

    pub async fn load(
        path: PathBuf,
        env: String,
        project: Option<String>,
        prompt_project: bool,
    ) -> Result<super::Config, anyhow::Error> {
        if !path.exists() {
            warn!("Config doesn't exists, loading default");
            return Ok(super::Config::default());
        }

        let content = tokio::fs::read_to_string(path).await?;
        let config_envs: ConfigEnvs = serde_yaml::from_str(&content)?;

        let mut config = super::Config::default();

        let config_env = config_envs.get(&env).cloned().unwrap_or_default();
        let config_default = config_envs.get("__default__").cloned().unwrap_or_default();

        config.runner_url = config_env.runner_url.or(config_default.runner_url);
        config.token = config_env.token.or(config_default.token);
        config.ws_runner_url = config_env.ws_runner_url.or(config_default.ws_runner_url);
        config.default_project = config_env
            .default_project
            .or(config_default.default_project);

        let config_prompt_project = config_env
            .prompt_project
            .or(config_default.prompt_project)
            .unwrap_or(false);

        let project = if let Some(project) = project {
            Some(Some(project))
        } else if prompt_project || (config_prompt_project && config.default_project.is_none()) {
            Some(None)
        } else {
            None
        };
        config.project_arg = project;

        Ok(config)
    }
}
