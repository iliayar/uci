use std::collections::HashMap;

use std::path::PathBuf;

use super::LoadConfigError;

const PROJECTS_CONFIG: &str = "projects.yaml";

#[derive(Debug)]
pub struct Projects {
    projects: HashMap<String, super::Project>,
}

impl Projects {
    pub async fn load(configs_root: PathBuf) -> Result<Projects, LoadConfigError> {
        raw::parse(configs_root.join(PROJECTS_CONFIG)).await
    }

    pub fn get(&self, project: &str) -> Option<&super::Project> {
        self.projects.get(project)
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::lib::{config, utils};

    #[derive(Deserialize, Serialize)]
    struct Project {
        path: String,
    }

    #[derive(Deserialize, Serialize)]
    struct Projects {
        projects: HashMap<String, Project>,
    }

    pub async fn parse(config_path: PathBuf) -> Result<super::Projects, super::LoadConfigError> {
        let project_raw = tokio::fs::read_to_string(config_path.clone()).await?;
        let data: Projects = serde_yaml::from_str(&project_raw)?;

        let mut projects = HashMap::new();

        for (id, Project { path }) in data.projects.into_iter() {
            projects.insert(
                id.clone(),
                config::Project::load(
                    id.clone(),
                    utils::abs_or_rel_to_file(path, config_path.clone()),
                )
                .await?,
            );
        }

        Ok(super::Projects { projects })
    }
}
