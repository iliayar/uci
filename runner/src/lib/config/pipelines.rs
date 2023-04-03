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
    use log::*;

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
        jobs: HashMap<String, Job>,
        links: Option<HashMap<String, String>>,
    }

    #[derive(Deserialize, Serialize)]
    struct Job {
        needs: Option<Vec<String>>,
        steps: Option<Vec<Step>>,
    }

    #[derive(Deserialize, Serialize)]
    struct Step {
        #[serde(rename = "type")]
        t: Option<Type>,
        script: Option<String>,
        interpreter: Option<Vec<String>>,
	image: Option<String>,
	networks: Option<Vec<String>>,
	volumes: Option<HashMap<String, String>>,
    }

    #[derive(Deserialize, Serialize, Clone, Copy)]
    enum Type {
        #[serde(rename = "script")]
        Script,
    }

    impl TryFrom<Pipeline> for common::Pipeline {
        type Error = super::LoadConfigError;

        fn try_from(value: Pipeline) -> Result<Self, Self::Error> {
            let jobs: Result<HashMap<_, _>, super::LoadConfigError> = value
                .jobs
                .into_iter()
                .map(|(k, v)| {
                    if v.steps.is_none() {
                        warn!("steps in job {} is not specified", k);
                    }

                    let steps: Result<Vec<common::Step>, super::LoadConfigError> = v
                        .steps
                        .unwrap_or_default()
                        .into_iter()
                        .map(|step| step.try_into())
                        .collect();
                    let steps = steps?;

                    let job = common::Job {
                        needs: v.needs.unwrap_or_default(),
                        steps,
                    };

                    Ok((k, job))
                })
                .collect();
            let jobs = jobs?;

            Ok(common::Pipeline {
                jobs,
                links: value.links.unwrap_or_default(),
		networks: Default::default(),
		volumes: Default::default(),
            })
        }
    }

    impl TryFrom<Step> for common::Step {
        type Error = super::LoadConfigError;

        fn try_from(value: Step) -> Result<Self, Self::Error> {
            match get_type(&value)? {
                Type::Script => {
                    let config = common::RunShellConfig {
                        script: value
                            .script
                            .ok_or(anyhow!("'script' step requires 'scipt' field"))?,
                        docker_image: value.image,
                        interpreter: value.interpreter,
			volumes: value.volumes.unwrap_or_default(),
			networks: value.networks.unwrap_or_default(),
                    };
                    Ok(common::Step::RunShell(config))
                }
            }
        }
    }

    async fn load_pipeline(path: PathBuf) -> Result<common::Pipeline, super::LoadConfigError> {
        config::utils::load_file::<Pipeline, _>(path).await
    }

    fn get_type(step: &Step) -> Result<Type, super::LoadConfigError> {
        if let Some(t) = step.t {
            Ok(t)
        } else if let Some(t) = guess_type(step) {
            Ok(t)
        } else {
            Err(anyhow!("Type is not specified for step, cannot guess type").into())
        }
    }

    fn guess_type(step: &Step) -> Option<Type> {
        if step.script.is_some() {
            Some(Type::Script)
        } else {
            None
        }
    }
}
