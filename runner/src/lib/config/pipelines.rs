use std::{collections::HashMap, path::PathBuf};

use super::LoadConfigError;

const PIPELINES_CONFIG: &str = "pipelines.yaml";

#[derive(Debug)]
pub struct Pipelines {
    pipelines: HashMap<String, common::Pipeline>,
}

impl Pipelines {
    pub async fn load(project_root: PathBuf) -> Result<Pipelines, LoadConfigError> {
        raw::parse(project_root.join(PIPELINES_CONFIG)).await
    }

    pub fn get(&self, pipeline: &str) -> Option<&common::Pipeline> {
        self.pipelines.get(pipeline)
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::lib::{config, utils};

    use anyhow::anyhow;

    #[derive(Deserialize, Serialize)]
    struct Pipelines {
        pipelines: HashMap<String, PipelineLocation>,
    }

    #[derive(Deserialize, Serialize)]
    struct PipelineLocation {
        path: String,
    }

    pub async fn parse(config_root: PathBuf) -> Result<super::Pipelines, super::LoadConfigError> {
        let content = tokio::fs::read_to_string(config_root.clone()).await?;
        let data: Pipelines = serde_yaml::from_str(&content)?;

        let mut pipelines = HashMap::new();
        for (id, PipelineLocation { path }) in data.pipelines.into_iter() {
            pipelines.insert(
                id,
                load_pipeline(utils::abs_or_rel_to_file(path, config_root.clone())).await?,
            );
        }

        Ok(super::Pipelines { pipelines })
    }

    #[derive(Deserialize, Serialize)]
    struct Pipeline {
        steps: Vec<Step>,
    }

    #[derive(Deserialize, Serialize)]
    struct Step {
        #[serde(rename = "type")]
        t: Type,
        script: Option<String>,
        interpreter: Option<Vec<String>>,
    }

    #[derive(Deserialize, Serialize)]
    enum Type {
        #[serde(rename = "script")]
        Script,
    }

    impl TryFrom<Pipeline> for common::Pipeline {
        type Error = super::LoadConfigError;

        fn try_from(value: Pipeline) -> Result<Self, Self::Error> {
            let mut steps = Vec::<common::Step>::new();

            for step_raw in value.steps.into_iter() {
                match step_raw.t {
                    Type::Script => {
                        let config = common::RunShellConfig {
                            script: step_raw
                                .script
                                .ok_or(anyhow!("'script' step requires 'scipt' field"))?,
                            docker_image: None,
                            interpreter: step_raw.interpreter,
                        };
                        steps.push(common::Step::RunShell(config));
                    }
                }
            }

            Ok(common::Pipeline { steps })
        }
    }

    async fn load_pipeline(path: PathBuf) -> Result<common::Pipeline, super::LoadConfigError> {
        config::utils::load_file::<Pipeline, _>(path).await
    }
}
