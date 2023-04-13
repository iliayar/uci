use crate::lib::git;

use std::collections::{HashMap, HashSet};

use std::path::PathBuf;

use anyhow::anyhow;
use log::*;

use super::LoadConfigError;

#[derive(Debug, Default)]
pub struct Actions {
    actions: HashMap<String, Vec<Trigger>>,
}

#[derive(Debug, Clone)]
pub enum ServiceAction {
    Deploy,
}

#[derive(Debug)]
pub struct Trigger {
    on: TriggerType,
    run_pipelines: Option<Vec<String>>,
    services: Option<HashMap<String, ServiceAction>>,
    reload_config: bool,
    reload_project: bool,
}

#[derive(Debug)]
pub enum TriggerType {
    Call {
        project_id: String,
        trigger_id: String,
    },
    ProjectReload {
        project_id: String,
    },
    ReposUpdated {
        patterns: HashMap<String, Vec<regex::Regex>>,
    },
    ConfigReload,
}

pub enum Event {
    ProjectReloaded {
        project_id: String,
    },
    Call {
        project_id: String,
        trigger_id: String,
    },
    RepoUpdate {
        diffs: super::ReposDiffs,
    },
    ConfigReload,
}

pub const ACTIONS_CONFIG: &str = "actions.yaml";

impl Actions {
    pub async fn load<'a>(context: &super::State<'a>) -> Result<Actions, LoadConfigError> {
        raw::load(context).await
    }

    pub async fn get_matched_actions(
        &self,
        event: &Event,
    ) -> Result<super::ProjectMatchedActions, super::ExecutionError> {
        let reload_config = self
            .get_actions(event, &|trigger| Some(trigger.reload_config))
            .await?
            .into_iter()
            .any(|v| v);
        let reload_project = self
            .get_actions(event, &|trigger| Some(trigger.reload_project))
            .await?
            .into_iter()
            .any(|v| v);
        let run_pipelines: HashSet<String> = self
            .get_actions(event, &|trigger| trigger.run_pipelines.clone())
            .await?
            .into_iter()
            .map(|v| v.into_iter())
            .flatten()
            .collect();
        let services: HashMap<String, super::ServiceAction> = self
            .get_actions(event, &|case| case.services.clone())
            .await?
            .into_iter()
            .map(|m| m.into_iter())
            .flatten()
            .collect();
        Ok(super::ProjectMatchedActions {
            reload_config,
            reload_project,
            run_pipelines,
            services,
        })
    }

    pub async fn get_actions<T>(
        &self,
        event: &Event,
        f: &impl Fn(&super::Trigger) -> Option<T>,
    ) -> Result<Vec<T>, super::ExecutionError> {
        let mut actions = Vec::new();
        for (action_id, triggers) in self.actions.iter() {
            for (i, case) in triggers.iter().enumerate() {
                if case.on.check_matched(event).await {
                    info!("Match trigger {} on action {}", i, action_id);
                    if let Some(value) = f(case) {
                        actions.push(value);
                    }
                }
            }
        }
        Ok(actions)
    }
}

impl TriggerType {
    async fn check_matched(&self, event: &Event) -> bool {
        match self {
            TriggerType::Call {
                project_id,
                trigger_id,
            } => match event {
                Event::Call {
                    project_id: event_project_id,
                    trigger_id: event_trigger_id,
                } => project_id == event_project_id && trigger_id == event_trigger_id,
                _ => false,
            },
            TriggerType::ProjectReload { project_id } => match event {
                Event::ProjectReloaded {
                    project_id: event_project_id,
                } => project_id == event_project_id,
                _ => false,
            },
            TriggerType::ReposUpdated { patterns } => match event {
                Event::RepoUpdate { diffs } => {
                    for (repo_id, patterns) in patterns.iter() {
                        if let Some(repo_diffs) = diffs.get(repo_id) {
                            for diff in repo_diffs.iter() {
                                for pattern in patterns.iter() {
                                    if pattern.is_match(diff) {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                    return false;
                }
                _ => false,
            },
            TriggerType::ConfigReload => match event {
                Event::ConfigReload => true,
                _ => false,
            },
        }
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::lib::config;

    use anyhow::anyhow;

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Actions {
        actions: HashMap<String, Vec<Trigger>>,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    enum TriggerType {
        #[serde(rename = "call")]
        Call,

        #[serde(rename = "project_reload")]
        ProjectReload,

        #[serde(rename = "changed")]
        FileChanged,

        #[serde(rename = "config_reload")]
        ConfigReload,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    enum ServiceAction {
        #[serde(rename = "deploy")]
        Deploy,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Trigger {
        #[serde(rename = "on")]
        on: TriggerType,
        run_pipelines: Option<Vec<String>>,
        services: Option<HashMap<String, ServiceAction>>,
        reload_config: Option<serde_yaml::Value>,
        reload_project: Option<serde_yaml::Value>,
        changes: Option<HashMap<String, Vec<String>>>,
    }

    impl config::LoadRawSync for Actions {
        type Output = super::Actions;

        fn load_raw(
            self,
            context: &config::State,
        ) -> Result<Self::Output, config::LoadConfigError> {
            Ok(super::Actions {
                actions: self.actions.load_raw(context)?,
            })
        }
    }

    impl config::LoadRawSync for Trigger {
        type Output = super::Trigger;

        fn load_raw(self, state: &config::State) -> Result<Self::Output, config::LoadConfigError> {
            if let TriggerType::ProjectReload = self.on {
                if self.reload_config.is_some() {
                    return Err(anyhow!(
                        "Trigger on project_reload is disallowed with reload_config action"
                    )
                    .into());
                }

                if self.reload_project.is_some() {
                    return Err(anyhow!(
                        "Trigger on project_reload is disallowed with reload_project action"
                    )
                    .into());
                }
            }

            let project_info: &config::ProjectInfo = state.get()?;
            let project_id = project_info.id.clone();
            let trigger_id: String = state.get_named("_id").cloned()?;

            let on = match self.on {
                TriggerType::Call => super::TriggerType::Call {
                    project_id: project_id.clone(),
                    trigger_id: trigger_id.clone(),
                },
                TriggerType::ProjectReload => super::TriggerType::ProjectReload {
                    project_id: project_id.clone(),
                },
                TriggerType::FileChanged => {
                    let changes: Result<HashMap<_, _>, super::LoadConfigError> = self
                        .changes
                        .ok_or(anyhow!("changes field required for on: changed"))?
                        .into_iter()
                        .map(|(r, ps)| {
                            let nps: Result<Vec<_>, super::LoadConfigError> =
                                ps.into_iter().map(|p| Ok(regex::Regex::new(&p)?)).collect();
                            Ok((r, nps?))
                        })
                        .collect();
                    super::TriggerType::ReposUpdated { patterns: changes? }
                }
                TriggerType::ConfigReload => super::TriggerType::ConfigReload,
            };

            Ok(super::Trigger {
                run_pipelines: self.run_pipelines,
                services: self.services.load_raw(state)?,
                reload_config: self.reload_config.is_some(),
                reload_project: self.reload_project.is_some(),
                on,
            })
        }
    }

    impl config::LoadRawSync for ServiceAction {
        type Output = super::ServiceAction;

        fn load_raw(self, state: &config::State) -> Result<Self::Output, config::LoadConfigError> {
            Ok(match self {
                ServiceAction::Deploy => super::ServiceAction::Deploy,
            })
        }
    }

    pub async fn load<'a>(
        state: &config::State<'a>,
    ) -> Result<super::Actions, super::LoadConfigError> {
        let projects_info: &config::ProjectInfo = state.get()?;
        let path = projects_info.path.join(super::ACTIONS_CONFIG);
        if !path.exists() {
            return Ok(Default::default());
        }
        config::load_sync::<Actions>(path.clone(), state)
            .await
            .map_err(|err| anyhow!("Failed to load actions from {:?}: {}", path, err).into())
    }
}
