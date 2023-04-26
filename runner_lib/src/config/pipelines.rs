use std::collections::HashMap;

use anyhow::anyhow;
use common::state::State;

#[derive(Debug, Default)]
pub struct Pipelines {
    pipelines: HashMap<String, common::Pipeline>,
}

pub struct PipelinesDescription {
    pub pipelines: Vec<PipelineDescription>,
}

pub struct PipelineDescription {
    pub name: String,
}

impl Pipelines {
    pub async fn load<'a>(state: &State<'a>) -> Result<Pipelines, anyhow::Error> {
        raw::load(state)
            .await
            .map_err(|err| anyhow!("Failed to pipelines: {}", err))
    }

    pub fn get(&self, pipeline: &str) -> Option<&common::Pipeline> {
        self.pipelines.get(pipeline)
    }

    pub async fn list_pipelines(&self) -> PipelinesDescription {
        let mut pipelines = Vec::new();
        for (pipeline_id, pipeline) in self.pipelines.iter() {
            pipelines.push(PipelineDescription {
                name: pipeline_id.clone(),
            });
        }
        PipelinesDescription { pipelines }
    }
}

pub const PIPELINES_CONFIG: &str = "pipelines.yaml";

mod raw {
    use std::collections::HashMap;

    use common::state::State;
    use serde::{Deserialize, Serialize};

    use crate::{config, utils};

    use anyhow::anyhow;

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
        stages: Option<HashMap<String, Stage>>,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Job {
        needs: Option<Vec<String>>,
        steps: Option<Vec<Step>>,
        stage: Option<String>,
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

    #[derive(Deserialize, Serialize, Clone, Copy)]
    #[serde(deny_unknown_fields)]
    struct Stage {
        on_overlap: StageOverlapPolicy,
    }

    #[derive(Deserialize, Serialize, Clone, Copy)]
    #[serde(deny_unknown_fields)]
    enum StageOverlapPolicy {
        #[serde(rename = "ignore")]
        Ignore,

        #[serde(rename = "displace")]
        Displace,

        #[serde(rename = "wait")]
        Wait,
    }

    #[async_trait::async_trait]
    impl config::LoadRaw for Pipelines {
        type Output = super::Pipelines;

        async fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            let mut pipelines: HashMap<String, common::Pipeline> = HashMap::new();
            for (id, PipelineLocation { path }) in self.pipelines.into_iter() {
                let project_info: &config::ProjectInfo = state.get()?;

                let mut state = state.clone();
                state.set_named("_id", &id);

                let pipeline_path = utils::eval_rel_path(&state, path, project_info.path.clone())?;
                let pipeline: Result<common::Pipeline, anyhow::Error> =
                    config::load_sync::<Pipeline>(pipeline_path.clone(), &state)
                        .await
                        .map_err(|err| {
                            anyhow!("Failed to load pipeline from {:?}: {}", pipeline_path, err)
                        });
                pipelines.insert(id, pipeline?);
            }

            Ok(super::Pipelines { pipelines })
        }
    }

    impl config::LoadRawSync for StageOverlapPolicy {
        type Output = common::OverlapStrategy;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            Ok(match self {
                StageOverlapPolicy::Ignore => common::OverlapStrategy::Ignore,
                StageOverlapPolicy::Displace => common::OverlapStrategy::Displace,
                StageOverlapPolicy::Wait => common::OverlapStrategy::Wait,
            })
        }
    }

    impl config::LoadRawSync for Stage {
        type Output = common::Stage;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            Ok(common::Stage {
                overlap_strategy: self
                    .on_overlap
                    .load_raw(state)
                    .unwrap_or(common::OverlapStrategy::Wait),
            })
        }
    }

    impl config::LoadRawSync for Pipeline {
        type Output = common::Pipeline;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            let links = config::utils::substitute_vars_dict(state, self.links.unwrap_or_default())?;
            let id: String = state.get_named("_id").cloned()?;

            let default_stage = || {
                (
                    worker_lib::executor::DEFEAULT_STAGE.to_string(),
                    common::Stage {
                        overlap_strategy: common::OverlapStrategy::Wait,
                    },
                )
            };

            let stages: HashMap<String, common::Stage> =
                if let Some(stages) = self.stages.load_raw(state)? {
                    if stages.is_empty() {
                        HashMap::from_iter([default_stage()])
                    } else {
                        stages
                    }
                } else {
                    HashMap::from_iter([default_stage()])
                };

            Ok(common::Pipeline {
                links,
                id,
                stages,
                jobs: self.jobs.load_raw(state)?,
                networks: Default::default(),
                volumes: Default::default(),
            })
        }
    }

    impl config::LoadRawSync for Job {
        type Output = common::Job;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            Ok(common::Job {
                needs: self.needs.unwrap_or_default(),
                steps: self.steps.unwrap_or_default().load_raw(state)?,
                stage: self.stage,
            })
        }
    }

    impl config::LoadRawSync for Step {
        type Output = common::Step;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            match get_type(&self)? {
                Type::Script => {
                    let networks = config::utils::get_networks_names(
                        state,
                        self.networks.unwrap_or_default(),
                    )?;
                    let volumes =
                        config::utils::get_volumes_names(state, self.volumes.unwrap_or_default())?;

                    let config = common::RunShellConfig {
                        script: self
                            .script
                            .ok_or_else(|| anyhow!("'script' step requires 'script' field"))?,
                        docker_image: self.image,
                        interpreter: self.interpreter,
                        env: config::utils::substitute_vars_dict(
                            state,
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

    fn get_type(step: &Step) -> Result<Type, anyhow::Error> {
        if let Some(t) = step.t {
            Ok(t)
        } else if let Some(t) = guess_type(step) {
            Ok(t)
        } else {
            Err(anyhow!("Type is not specified for step, cannot guess type"))
        }
    }

    fn guess_type(step: &Step) -> Option<Type> {
        if step.script.is_some() {
            Some(Type::Script)
        } else {
            None
        }
    }

    pub async fn load<'a>(state: &State<'a>) -> Result<super::Pipelines, anyhow::Error> {
        let project_info: &config::ProjectInfo = state.get()?;
        let path = project_info.path.join(super::PIPELINES_CONFIG);
        if !path.exists() {
            return Ok(Default::default());
        }
        config::load::<Pipelines>(path.clone(), state)
            .await
            .map_err(|err| anyhow!("Failed to load pipelines from {:?}: {}", path, err))
    }
}
