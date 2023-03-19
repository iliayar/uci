use std::{collections::HashMap, path::PathBuf};

use log::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use serde_yaml;
use tokio::{
    fs::{self, File},
    io::{AsyncBufReadExt, BufReader},
};

#[derive(Debug)]
pub struct Config {
    pub repos: HashMap<String, Repo>,
    pub projects: HashMap<String, Project>,
}

#[derive(Debug)]
pub struct Repo {
    pub source: String,
    pub branch: String,
}

#[derive(Debug)]
pub struct Project {
    path: PathBuf,
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

        let repos = Config::load_repos(configs_root.join("repos.yaml")).await?;
        let projects = Config::load_projects(configs_root.join("projects.yaml")).await?;

        Ok(Config { repos, projects })
    }

    async fn load_repos(repos_config: PathBuf) -> Result<HashMap<String, Repo>, ConfigError> {
        #[derive(Serialize, Deserialize)]
        struct RawRepo {
            id: String,
            source: String,
            branch: Option<String>,
        }

        #[derive(Serialize, Deserialize)]
        struct RawRepos {
            repos: Vec<RawRepo>,
        }

        let repos_raw = fs::read_to_string(repos_config).await?;
        let data: RawRepos = serde_yaml::from_str(&repos_raw)?;

        let mut repos = HashMap::new();
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
        projects_config: PathBuf,
    ) -> Result<HashMap<String, Project>, ConfigError> {

        #[derive(Serialize, Deserialize)]
	struct RawProject {
	    id: String,
	    path: PathBuf,
	}

        #[derive(Serialize, Deserialize)]
	struct RawProjects {
	    projects: Vec<RawProject>,
	}

	let project_raw = fs::read_to_string(projects_config).await?;
	let data: RawProjects = serde_yaml::from_str(&project_raw)?;

        let mut projects = HashMap::new();
	for RawProject { id, path } in data.projects.into_iter() {
	    projects.insert(id, Project {
		path,
	    });
	}

        Ok(projects)
    }
}
