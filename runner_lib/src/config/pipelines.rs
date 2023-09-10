use std::collections::HashMap;

use anyhow::{anyhow, Result};
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
    pub fn merge(self, other: Pipelines) -> Result<Pipelines> {
        let mut pipelines = HashMap::new();

        for (id, pipeline) in self
            .pipelines
            .into_iter()
            .chain(other.pipelines.into_iter())
        {
            if pipelines.contains_key(&id) {
                return Err(anyhow!("Pipeline {} duplicates", id));
            }
            pipelines.insert(id, pipeline);
        }

        Ok(Pipelines { pipelines })
    }

    pub async fn get<'a>(
        &self,
        state: &State<'a>,
        pipeline: impl AsRef<str>,
    ) -> Result<common::Pipeline> {
        self.pipelines
            .get(pipeline.as_ref())
            .cloned()
            .ok_or_else(|| anyhow!("No such pipeline: {}", pipeline.as_ref()))
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

pub mod raw {
    use crate::config;

    use dynconf::*;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    use anyhow::{anyhow, Result};

    #[derive(Deserialize, Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct Pipelines {
        pipelines: HashMap<String, util::Dyn<Pipeline>>,
    }

    #[derive(Deserialize, Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    struct Pipeline {
        jobs: HashMap<String, Job>,
        links: Option<HashMap<String, util::DynString>>,
        stages: Option<HashMap<String, Stage>>,
        integrations: Option<HashMap<String, util::DynAny>>,
    }

    #[derive(Deserialize, Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    struct JobCommon {
        needs: Option<Vec<String>>,
        stage: Option<String>,
        // TODO: Make it lazy
        enabled: Option<util::Dyn<bool>>,
    }

    #[derive(Deserialize, Serialize, Clone, Debug)]
    #[serde(untagged)]
    enum Job {
        JobWithSingleStep {
            #[serde(flatten)]
            common: JobCommon,

            #[serde(flatten)]
            step: Step,
        },
        JobWithSteps {
            #[serde(flatten)]
            common: JobCommon,

            steps: Vec<Step>,
        },
    }

    #[derive(Deserialize, Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    #[serde(tag = "type")]
    enum Step {
        #[serde(rename = "script")]
        Script {
            script: String,
            interpreter: Option<Vec<String>>,
            image: Option<String>,
            networks: Option<Vec<String>>,
            volumes: Option<HashMap<String, util::DynString>>,
            env: Option<HashMap<String, util::DynString>>,
        },
        #[serde(rename = "build")]
        BuildImage {
            path: util::DynPath,
            image: String,
            dockerfile: Option<String>,
        },
    }

    #[derive(Deserialize, Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    struct Stage {
        on_overlap: Option<StageOverlapPolicy>,
        repos: Option<StageRepos>,
    }

    #[derive(Deserialize, Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields, untagged)]
    enum StageRepos {
        Exact(HashMap<String, RepoLockStrategy>),
        All(RepoLockStrategy),
    }

    #[derive(Deserialize, Serialize, Clone, Debug)]
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

    #[derive(Deserialize, Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    enum RepoLockStrategy {
        #[serde(rename = "lock")]
        Lock,

        #[serde(rename = "unlock")]
        Unlock,
    }

    #[async_trait::async_trait]
    impl util::DynValue for Pipelines {
        type Target = super::Pipelines;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            Ok(super::Pipelines {
                pipelines: self.pipelines.load(state).await?,
            })
        }
    }

    #[async_trait::async_trait]
    impl util::DynValue for RepoLockStrategy {
        type Target = common::RepoLockStrategy;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            Ok(match self {
                RepoLockStrategy::Lock => common::RepoLockStrategy::Lock,
                RepoLockStrategy::Unlock => common::RepoLockStrategy::Unlock,
            })
        }
    }

    #[async_trait::async_trait]
    impl util::DynValue for StageRepos {
        type Target = common::StageRepos;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            Ok(match self {
                StageRepos::Exact(repos) => common::StageRepos::Exact(repos.load(state).await?),
                StageRepos::All(strat) => common::StageRepos::All(strat.load(state).await?),
            })
        }
    }

    #[async_trait::async_trait]
    impl util::DynValue for StageOverlapPolicy {
        type Target = common::OverlapStrategy;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            Ok(match self {
                StageOverlapPolicy::Ignore => common::OverlapStrategy::Ignore,
                StageOverlapPolicy::Displace => common::OverlapStrategy::Displace,
                StageOverlapPolicy::Cancel => common::OverlapStrategy::Cancel,
                StageOverlapPolicy::Wait => common::OverlapStrategy::Wait,
            })
        }
    }

    #[async_trait::async_trait]
    impl util::DynValue for Stage {
        type Target = common::Stage;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            Ok(common::Stage {
                overlap_strategy: self
                    .on_overlap
                    .load(state)
                    .await?
                    .unwrap_or(common::OverlapStrategy::Wait),
                repos: self.repos.load(state).await?,
            })
        }
    }

    #[async_trait::async_trait]
    impl util::DynValue for Pipeline {
        type Target = common::Pipeline;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            let dynobj = config::utils::get_dyn_object(state)?;
            let id: String = dynobj._id.ok_or_else(|| anyhow!("No _id binding"))?;

            let links = self.links.load(state).await?.unwrap_or_default();

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
                if let Some(stages) = self.stages.load(state).await? {
                    if stages.is_empty() {
                        HashMap::from_iter([default_stage()])
                    } else {
                        stages
                    }
                } else {
                    HashMap::from_iter([default_stage()])
                };

            let integrations = self
                .integrations
                .load(state)
                .await?
                .unwrap_or_default()
                .into_iter()
                .map(|(k, v)| (k, v.to_json()))
                .collect();

            Ok(common::Pipeline {
                links,
                id,
                stages,
                integrations,
                jobs: self.jobs.load(state).await?,
                networks: Default::default(),
                volumes: Default::default(),
            })
        }
    }

    #[async_trait::async_trait]
    impl util::DynValue for Job {
        type Target = common::Job;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            let (common, steps) = match self {
                Job::JobWithSteps { common, steps } => (common, steps),
                Job::JobWithSingleStep { common, step } => (common, vec![step]),
            };
            Ok(common::Job {
                needs: common.needs.unwrap_or_default(),
                steps: steps.load(state).await?,
                stage: common.stage,
                enabled: common.enabled.load(state).await?.unwrap_or(true),
            })
        }
    }

    #[async_trait::async_trait]
    impl util::DynValue for Step {
        type Target = common::Step;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            let dynobj = config::utils::get_dyn_object(state)?;
            let services = dynobj
                .services
                .ok_or_else(|| anyhow!("No services binding"))?;

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
                    let networks: Result<Vec<String>> = networks
                        .unwrap_or_default()
                        .into_iter()
                        .map(|name: String| {
                            services
                                .networks
                                .get(&name)
                                .ok_or_else(|| anyhow!("Unknown network: {}", name))
                                .cloned()
                        })
                        .collect();

                    let mut volumes_res: HashMap<String, String> = HashMap::new();
                    for (mount_path, name) in volumes.load(state).await?.unwrap_or_default().into_iter() {
                        let name = if let Some(name) = services.volumes.get(&name) {
                            name.clone()
                        } else {
                            // name is path
                            name
                        };
			volumes_res.insert(name, mount_path);
                    }

                    let config = common::RunShellConfig {
                        docker_image: image,
                        env: env.load(state).await?.unwrap_or_default(),
                        volumes: volumes_res,
                        networks: networks?,
                        script,
                        interpreter,
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
                                path.load(state).await?.to_string_lossy().to_string(),
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
}
