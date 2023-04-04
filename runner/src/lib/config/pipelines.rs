use std::collections::HashMap;

use super::LoadConfigError;

#[derive(Debug)]
pub struct Pipelines {
    pipelines: HashMap<String, common::Pipeline>,
}

impl Pipelines {
    pub async fn load<'a>(context: &super::LoadContext<'a>) -> Result<Pipelines, LoadConfigError> {
        raw::load(context).await
    }

    pub fn get(&self, pipeline: &str) -> Option<&common::Pipeline> {
        self.pipelines.get(pipeline)
    }
}

mod raw {
    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};

    use crate::lib::{config, utils};

    use anyhow::anyhow;
    use log::*;

    const PIPELINES_CONFIG: &str = "pipelines.yaml";

    #[derive(Deserialize, Serialize)]
    struct Pipelines {
        pipelines: HashMap<String, PipelineLocation>,
    }

    #[derive(Deserialize, Serialize)]
    struct PipelineLocation {
        path: String,
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

    #[async_trait::async_trait]
    impl config::LoadRaw for Pipelines {
        type Output = super::Pipelines;

        async fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            let mut pipelines = HashMap::new();
            for (id, PipelineLocation { path }) in self.pipelines.into_iter() {
                let pipeline_path = utils::abs_or_rel_to_dir(path, context.project_root()?.clone());
                let pipeline = config::load_sync::<Pipeline>(pipeline_path, context).await?;
                pipelines.insert(id, pipeline);
            }

            Ok(super::Pipelines { pipelines })
        }
    }

    impl config::LoadRawSync for Pipeline {
        type Output = common::Pipeline;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            let links =
                config::utils::substitute_path_vars(context, self.links.unwrap_or_default())?;

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
            context: &config::LoadContext,
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
            context: &config::LoadContext,
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
        context: &config::LoadContext<'a>,
    ) -> Result<super::Pipelines, super::LoadConfigError> {
        config::load::<Pipelines>(context.project_root()?.join(PIPELINES_CONFIG), context).await
    }
}
