use std::collections::{HashMap, HashSet};

use common::state::State;
use log::*;

#[derive(Debug, Default)]
pub struct Actions {
    actions: HashMap<String, Vec<Trigger>>,
}

#[derive(Debug, Clone)]
pub enum ServiceAction {
    Deploy,
    Logs { follow: bool, tail: Option<usize> },
    Start { build: bool },
    Stop,
    Restart { build: bool },
}

impl ToString for ServiceAction {
    fn to_string(&self) -> String {
        match self {
            ServiceAction::Deploy => "deploy".to_string(),
            ServiceAction::Start { .. } => "start".to_string(),
            ServiceAction::Stop => "stop".to_string(),
            ServiceAction::Restart { .. } => "restart".to_string(),
            ServiceAction::Logs { .. } => "logs".to_string(),
        }
    }
}

#[derive(Debug)]
pub struct Trigger {
    on: TriggerType,
    run_pipelines: Option<Vec<String>>,
    services: Option<HashMap<String, ServiceAction>>,
}

#[derive(Debug)]
pub enum TriggerType {
    Call {
        project_id: String,
        trigger_id: String,
    },
    ReposUpdated {
        repo_id: String,
        patterns: Vec<regex::Regex>,
        exclude_patterns: Vec<regex::Regex>,
        exclude_commits: Vec<regex::Regex>,
    },
}

pub enum Event {
    Call {
        project_id: String,
        trigger_id: String,
    },
    RepoUpdate {
        repo_id: String,
        diffs: super::Diff,
    },
}

pub struct ActionsDescription {
    pub actions: Vec<ActionDescription>,
}

pub struct ActionDescription {
    pub name: String,
}

pub const ACTIONS_CONFIG: &str = "actions.yaml";

impl Actions {
    pub async fn load<'a>(state: &State<'a>) -> Result<Actions, anyhow::Error> {
        raw::load(state).await
    }

    pub async fn list_actions<'a>(&self) -> ActionsDescription {
        let mut actions = Vec::new();

        for (action_id, triggers) in self.actions.iter() {
            actions.push(ActionDescription {
                name: action_id.clone(),
            });
        }

        ActionsDescription { actions }
    }

    pub async fn get_matched_actions(
        &self,
        event: &Event,
    ) -> Result<super::EventActions, anyhow::Error> {
        let run_pipelines: HashSet<String> = self
            .get_actions(event, &|trigger| trigger.run_pipelines.clone())
            .await?
            .into_iter()
            .flat_map(|v| v.into_iter())
            .collect();
        let services: HashMap<String, super::ServiceAction> = self
            .get_actions(event, &|case| case.services.clone())
            .await?
            .into_iter()
            .flat_map(|m| m.into_iter())
            .collect();
        Ok(super::EventActions {
            run_pipelines,
            services,
        })
    }

    pub async fn get_actions<T>(
        &self,
        event: &Event,
        f: &impl Fn(&super::Trigger) -> Option<T>,
    ) -> Result<Vec<T>, anyhow::Error> {
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
            TriggerType::ReposUpdated {
                repo_id,
                patterns,
                exclude_patterns,
                exclude_commits,
            } => match event {
                Event::RepoUpdate {
                    repo_id: event_repo_id,
                    diffs,
                } => {
                    if repo_id != event_repo_id {
                        return false;
                    } else {
                        match diffs {
                            super::Diff::Changes {
                                changes,
                                commit_message,
                            } => {
                                for pattern in exclude_commits.iter() {
                                    if pattern.is_match(commit_message) {
                                        return false;
                                    }
                                }

                                for diff in changes.iter() {
                                    let mut matched = false;
                                    for pattern in patterns.iter() {
                                        if pattern.is_match(diff) {
                                            matched = true;
                                        }
                                    }

                                    for pattern in exclude_patterns.iter() {
                                        if pattern.is_match(diff) {
                                            matched = false;
                                        }
                                    }

                                    return matched;
                                }
                            }
                            super::Diff::Whole => {
                                return true;
                            }
                        }
                    }
                    false
                }
                _ => false,
            },
        }
    }
}

mod raw {
    use std::collections::HashMap;

    use common::state::State;
    use serde::{Deserialize, Serialize};

    use crate::config;

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

        #[serde(rename = "changed")]
        FileChanged,
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
        repo_id: Option<String>,
        changes: Option<Vec<String>>,
        exclude_changes: Option<Vec<String>>,
        exclude_commits: Option<Vec<String>>,
    }

    impl config::LoadRawSync for Actions {
        type Output = super::Actions;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            Ok(super::Actions {
                actions: self.actions.load_raw(state)?,
            })
        }
    }

    impl config::LoadRawSync for Trigger {
        type Output = super::Trigger;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            let project_info: &config::ProjectInfo = state.get()?;
            let project_id = project_info.id.clone();
            let trigger_id: String = state.get_named("_id").cloned()?;

            let on = match self.on {
                TriggerType::Call => super::TriggerType::Call {
                    project_id,
                    trigger_id,
                },
                TriggerType::FileChanged => {
                    let changes: Result<Vec<_>, anyhow::Error> = self
                        .changes
                        .ok_or_else(|| anyhow!("'changes' field required for on: changed"))?
                        .into_iter()
                        .map(|pattern| Ok(regex::Regex::new(&pattern)?))
                        .collect();
                    let exclude_changes: Result<Vec<_>, anyhow::Error> = self
                        .exclude_changes
                        .unwrap_or_default()
                        .into_iter()
                        .map(|pattern| Ok(regex::Regex::new(&pattern)?))
                        .collect();
                    let exclude_commits: Result<Vec<_>, anyhow::Error> = self
                        .exclude_commits
                        .unwrap_or_default()
                        .into_iter()
                        .map(|pattern| Ok(regex::Regex::new(&pattern)?))
                        .collect();
                    let repo_id = self
                        .repo_id
                        .ok_or_else(|| anyhow!("'repo_id' fieled required for on: changed"))?;
                    super::TriggerType::ReposUpdated {
                        repo_id,
                        patterns: changes?,
                        exclude_patterns: exclude_changes?,
                        exclude_commits: exclude_commits?,
                    }
                }
            };

            Ok(super::Trigger {
                run_pipelines: self.run_pipelines,
                services: self.services.load_raw(state)?,
                on,
            })
        }
    }

    impl config::LoadRawSync for ServiceAction {
        type Output = super::ServiceAction;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            Ok(match self {
                ServiceAction::Deploy => super::ServiceAction::Deploy,
            })
        }
    }

    pub async fn load<'a>(state: &State<'a>) -> Result<super::Actions, anyhow::Error> {
        let projects_info: &config::ProjectInfo = state.get()?;
        let path = projects_info.path.join(super::ACTIONS_CONFIG);
        if !path.exists() {
            return Ok(Default::default());
        }
        config::load_sync::<Actions>(path.clone(), state)
            .await
            .map_err(|err| anyhow!("Failed to load actions from {:?}: {}", path, err))
    }
}
