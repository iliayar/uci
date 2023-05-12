use std::{collections::HashMap, path::PathBuf};

use anyhow::anyhow;
use common::state::State;

#[derive(Debug, Default)]
pub struct Pipelines {
    pipelines: HashMap<String, PipelineLocation>,
}

#[derive(Debug)]
struct PipelineLocation {
    path: PathBuf,
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

    pub async fn get<'a>(
        &self,
        state: &State<'a>,
        pipeline: impl AsRef<str>,
    ) -> Result<common::Pipeline, anyhow::Error> {
        let location = self
            .pipelines
            .get(pipeline.as_ref())
            .ok_or_else(|| anyhow!("No such pipeline: {}", pipeline.as_ref()))?;

        let pipeline_id = pipeline.as_ref().to_string();
        let mut state = state.clone();
        state.set_named("pipeline_id", &pipeline_id);

        raw::load_pipeline(&state, pipeline, &location.path).await
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
    use std::{collections::HashMap, path::PathBuf};

    use common::state::State;
    use serde::{Deserialize, Serialize};

    use crate::{
        config::{self, Expr},
        utils,
    };

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
        integrations: Option<HashMap<String, serde_json::Value>>,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct JobCommon {
        needs: Option<Vec<String>>,
        stage: Option<String>,
        enabled: Option<Expr<bool>>,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(untagged)]
    enum Job {
        JobWithSteps {
            #[serde(flatten)]
            common: JobCommon,

            steps: Vec<Step>,
        },
        JobWithSingleStep {
            #[serde(flatten)]
            common: JobCommon,

            #[serde(flatten)]
            step: Step,
        },
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    #[serde(tag = "type")]
    enum Step {
        #[serde(rename = "script")]
        Script {
            script: String,
            interpreter: Option<Vec<String>>,
            image: Option<String>,
            networks: Option<Vec<String>>,
            volumes: Option<HashMap<String, String>>,
            env: Option<HashMap<String, String>>,
        },
        #[serde(rename = "build")]
        BuildImage {
            path: config::AbsPath,
            image: String,
            dockerfile: Option<String>,
        },
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Stage {
        on_overlap: StageOverlapPolicy,
        repos: Option<StageRepos>,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields, untagged)]
    enum StageRepos {
        Exact(HashMap<String, RepoLockStrategy>),
        All(RepoLockStrategy),
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    enum StageOverlapPolicy {
        #[serde(rename = "ignore")]
        Ignore,

        #[serde(rename = "displace")]
        Displace,

        #[serde(rename = "cancel")]
        Cancel,

        #[serde(rename = "wait")]
        Wait,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    enum RepoLockStrategy {
        #[serde(rename = "lock")]
        Lock,

        #[serde(rename = "unlock")]
        Unlock,
    }

    impl config::LoadRawSync for PipelineLocation {
        type Output = super::PipelineLocation;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            let project_info: &config::ProjectInfo = state.get()?;
            let path = utils::eval_rel_path(state, self.path, project_info.path.clone())?;
            Ok(super::PipelineLocation { path })
        }
    }

    impl config::LoadRawSync for Pipelines {
        type Output = super::Pipelines;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            Ok(super::Pipelines {
                pipelines: self.pipelines.load_raw(state)?,
            })
        }
    }

    impl config::LoadRawSync for RepoLockStrategy {
        type Output = common::RepoLockStrategy;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            Ok(match self {
                RepoLockStrategy::Lock => common::RepoLockStrategy::Lock,
                RepoLockStrategy::Unlock => common::RepoLockStrategy::Unlock,
            })
        }
    }

    impl config::LoadRawSync for StageRepos {
        type Output = common::StageRepos;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            Ok(match self {
                StageRepos::Exact(repos) => common::StageRepos::Exact(repos.load_raw(state)?),
                StageRepos::All(strat) => common::StageRepos::All(strat.load_raw(state)?),
            })
        }
    }

    impl config::LoadRawSync for StageOverlapPolicy {
        type Output = common::OverlapStrategy;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            Ok(match self {
                StageOverlapPolicy::Ignore => common::OverlapStrategy::Ignore,
                StageOverlapPolicy::Displace => common::OverlapStrategy::Displace,
                StageOverlapPolicy::Cancel => common::OverlapStrategy::Cancel,
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
                repos: self.repos.load_raw(state)?,
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
                        repos: None,
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

            let integrations = self.integrations.unwrap_or_default();
            let integrations: Result<HashMap<String, serde_json::Value>, anyhow::Error> =
                integrations
                    .into_iter()
                    .map(|(k, v)| Ok((k, config::utils::substitute_vars_json(state, v)?)))
                    .collect();

            Ok(common::Pipeline {
                links,
                id,
                stages,
                jobs: self.jobs.load_raw(state)?,
                networks: Default::default(),
                volumes: Default::default(),
                integrations: integrations?,
            })
        }
    }

    impl config::LoadRawSync for Job {
        type Output = common::Job;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            let (common, steps) = match self {
                Job::JobWithSteps { common, steps } => (common, steps),
                Job::JobWithSingleStep { common, step } => (common, vec![step]),
            };
            Ok(common::Job {
                needs: common.needs.unwrap_or_default(),
                steps: steps.load_raw(state)?,
                stage: common.stage,
                enabled: common.enabled.load_raw(state)?.unwrap_or(true),
            })
        }
    }

    impl config::LoadRawSync for Step {
        type Output = common::Step;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            match self {
                Step::Script {
                    networks,
                    volumes,
                    script,
                    image,
                    interpreter,
                    env,
                    ..
                } => {
                    let networks =
                        config::utils::get_networks_names(state, networks.unwrap_or_default())?;
                    let volumes =
                        config::utils::get_volumes_names(state, volumes.unwrap_or_default())?;

                    let config = common::RunShellConfig {
                        docker_image: image,
                        env: config::utils::substitute_vars_dict(state, env.unwrap_or_default())?,
                        script,
                        interpreter,
                        volumes,
                        networks,
                    };
                    Ok(common::Step::RunShell(config))
                }
                Step::BuildImage {
                    image,
                    path,
                    dockerfile,
                } => {
                    let config = common::BuildImageConfig {
                        tag: None,
                        source: Some(common::BuildImageConfigSource {
                            path: common::BuildImageConfigSourcePath::Directory(
                                path.load_raw(state)?.to_string_lossy().to_string(),
                            ),
                            dockerfile,
                        }),
                        image,
                    };

                    Ok(common::Step::BuildImage(config))
                }
            }
        }
    }

    pub async fn load_pipeline<'a>(
        state: &State<'a>,
        id: impl AsRef<str>,
        path: &PathBuf,
    ) -> Result<common::Pipeline, anyhow::Error> {
        let mut state = state.clone();
        let id = id.as_ref().to_string();
        state.set_named("_id", &id);
        config::load_sync::<Pipeline>(path.clone(), &state)
            .await
            .map_err(|err| anyhow!("Failed to load pipeline from {:?}: {}", path, err))
    }

    pub async fn load<'a>(state: &State<'a>) -> Result<super::Pipelines, anyhow::Error> {
        let project_info: &config::ProjectInfo = state.get()?;
        let path = project_info.path.join(super::PIPELINES_CONFIG);
        if !path.exists() {
            return Ok(Default::default());
        }
        config::load_sync::<Pipelines>(path.clone(), state)
            .await
            .map_err(|err| anyhow!("Failed to load pipelines from {:?}: {}", path, err))
    }
}
