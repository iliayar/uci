use std::{collections::HashMap, path::PathBuf};

use log::*;
use serde::{Serialize, Deserialize};
use thiserror::Error;

use serde_yaml;
use tokio::{fs::{File, self}, io::{BufReader, AsyncBufReadExt}};

#[derive(Debug)]
pub struct Config {
    pub repos: HashMap<String, Repo>,
}

#[derive(Debug)]
pub struct Repo {
    pub source: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO Error: {0}")]
    IOError(#[from] tokio::io::Error),

    #[error("Yaml parsing error: {0}")]
    YamlParseError(#[from] serde_yaml::Error)
}

impl Config {
    pub async fn load(configs_root: PathBuf) -> Result<Config, ConfigError> {
	info!("Loading config");

        let repos = Config::load_repos(configs_root.join("repos.yaml")).await?;

        Ok(Config { repos })
    }

    async fn load_repos(repos_config: PathBuf) -> Result<HashMap<String, Repo>, ConfigError> {
	#[derive(Serialize, Deserialize)]
	struct RawRepo {
	    id: String,
	    source: String,
	}

	#[derive(Serialize, Deserialize)]
	struct RawRepos {
	    repos: Vec<RawRepo>
	}

	let repos_raw = fs::read_to_string(repos_config).await?;
	let data: RawRepos = serde_yaml::from_str(&repos_raw)?;

        let mut repos = HashMap::new();
	for RawRepo { id, source } in data.repos.into_iter() {
	    repos.insert(id, Repo { source });
	}

        Ok(repos)
    }
}
