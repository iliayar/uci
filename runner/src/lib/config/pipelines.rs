use std::collections::HashMap;

use super::LoadConfigError;

use anyhow::anyhow;

#[derive(Debug, Default)]
pub struct Pipelines {
    pipelines: HashMap<String, common::Pipeline>,
}

impl Pipelines {
    pub async fn load<'a>(context: &super::State<'a>) -> Result<Pipelines, LoadConfigError> {
        raw::load(context)
            .await
            .map_err(|err| anyhow!("Failed to pipelines: {}", err).into())
    }

    pub fn get(&self, pipeline: &str) -> Option<&common::Pipeline> {
        self.pipelines.get(pipeline)
    }
}

pub const PIPELINES_CONFIG: &str = "pipelines.yaml";

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::lib::{config, utils};

    use anyhow::anyhow;
    use log::*;

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Pipelines {
        pipelines: HashMap<String, PipelineLocation>,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct PipelineLocation {
        path: String,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Pipeline {
        jobs: HashMap<String, Job>,
        links: Option<HashMap<String, String>>,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Job {
        needs: Option<Vec<String>>,
        steps: Option<Vec<Step>>,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Step {
        #[serde(rename = "type")]
        t: Option<Type>,
        script: Option<String>,
        interpreter: Option<Vec<String>>,
        image: Option<String>,
        networks: Option<Vec<String>>,
        volumes: Option<HashMap<String, String>>,
        env: Option<HashMap<String, String>>,
    }

    #[derive(Deserialize, Serialize, Clone, Copy)]
    #[serde(deny_unknown_fields)]
    enum Type {
        #[serde(rename = "script")]
        Script,
    }

    #[async_trait::async_trait]
    impl config::LoadRaw for Pipelines {
        type Output = super::Pipelines;

        async fn load_raw(
            self,
            context: &config::State,
        ) -> Result<Self::Output, config::LoadConfigError> {
            let mut pipelines: HashMap<String, common::Pipeline> = HashMap::new();
            for (id, PipelineLocation { path }) in self.pipelines.into_iter() {
                let project_root: PathBuf = context.get_named("project_root").cloned()?;
                let pipeline_path = utils::eval_rel_path(context, path, project_root)?;
                let pipeline: Result<common::Pipeline, super::LoadConfigError> =
                    config::load_sync::<Pipeline>(pipeline_path.clone(), context)
                        .await
                        .map_err(|err| {
                            anyhow!("Failed to load pipeline from {:?}: {}", pipeline_path, err)
                                .into()
                        });
                pipelines.insert(id, pipeline?);
            }

            Ok(super::Pipelines { pipelines })
        }
    }

    impl config::LoadRawSync for Pipeline {
        type Output = common::Pipeline;

        fn load_raw(
            self,
            context: &config::State,
        ) -> Result<Self::Output, config::LoadConfigError> {
            let links =
                config::utils::substitute_vars_dict(context, self.links.unwrap_or_default())?;

            Ok(common::Pipeline {
                links,
                jobs: self.jobs.load_raw(context)?,
                networks: Default::default(),
                volumes: Default::default(),
            })
        }
    }

    impl config::LoadRawSync for Job {
        type Output = common::Job;

        fn load_raw(
            self,
            context: &config::State,
        ) -> Result<Self::Output, config::LoadConfigError> {
            Ok(common::Job {
                needs: self.needs.unwrap_or_default(),
                steps: self.steps.unwrap_or_default().load_raw(context)?,
            })
        }
    }

    impl config::LoadRawSync for Step {
        type Output = common::Step;

        fn load_raw(
            self,
            context: &config::State,
        ) -> Result<Self::Output, config::LoadConfigError> {
            match get_type(&self)? {
                Type::Script => {
                    let networks = config::utils::get_networks_names(
                        context,
                        self.networks.unwrap_or_default(),
                    )?;
                    let volumes = config::utils::get_volumes_names(
                        context,
                        self.volumes.unwrap_or_default(),
                    )?;

                    let config = common::RunShellConfig {
                        script: self
                            .script
                            .ok_or(anyhow!("'script' step requires 'script' field"))?,
                        docker_image: self.image,
                        interpreter: self.interpreter,
                        env: config::utils::substitute_vars_dict(
                            context,
                            self.env.unwrap_or_default(),
                        )?,
                        volumes,
                        networks,
                    };
                    Ok(common::Step::RunShell(config))
                }
            }
        }
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

    pub async fn load<'a>(
        context: &config::State<'a>,
    ) -> Result<super::Pipelines, super::LoadConfigError> {
        let project_root: PathBuf = context.get_named("project_root").cloned()?;
        let path = project_root.join(super::PIPELINES_CONFIG);
        if !path.exists() {
            return Ok(Default::default());
        }
        config::load::<Pipelines>(path.clone(), context)
            .await
            .map_err(|err| anyhow!("Failed to load pipelines from {:?}: {}", path, err).into())
    }
}
