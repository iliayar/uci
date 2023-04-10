use std::path::PathBuf;

use anyhow::anyhow;

#[derive(Debug, Default)]
pub struct Config {
    pub runner_url: Option<String>,
    pub ws_runner_url: Option<String>,
    pub token: Option<String>,
}

impl Config {
    pub async fn load(path: PathBuf, env: String) -> Result<Config, anyhow::Error> {
        raw::load(path, env).await
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
    }

    pub async fn load(path: PathBuf, env: String) -> Result<super::Config, anyhow::Error> {
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

        Ok(config)
    }
}
