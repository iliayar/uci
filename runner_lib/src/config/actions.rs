use crate::config;
use std::collections::{HashMap, HashSet};

use anyhow::anyhow;
use cron_tab::Cron;
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
    params: dynconf::Value,
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
    Cron {
        project_id: String,
        trigger_id: String,
        rule_pattern: String,
    },
}

pub enum Event {
    Call {
        project_id: String,
        trigger_id: String,
    },
    RepoUpdate {
        repo_id: String,
        diffs: config::repo::Diff,
    },
    Cron {
        project_id: String,
        trigger_id: String,
    },
}

pub struct ActionsDescription {
    pub actions: Vec<ActionDescription>,
}

pub struct ActionDescription {
    pub name: String,
}

impl Actions {
    pub fn merge(self, other: Actions) -> Result<Actions, anyhow::Error> {
        let mut actions = HashMap::new();

        for (id, action) in self.actions.into_iter().chain(other.actions.into_iter()) {
            if actions.contains_key(&id) {
                return Err(anyhow!("Action {} duplicate", id));
            }

            actions.insert(id, action);
        }

        Ok(Actions { actions })
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
    ) -> Result<config::project::EventActions, anyhow::Error> {
        let run_pipelines: HashSet<String> = self
            .get_actions(event, &|trigger| trigger.run_pipelines.clone())
            .await?
            .into_iter()
            .flat_map(|v| v.into_iter())
            .collect();
        let services: HashMap<String, ServiceAction> = self
            .get_actions(event, &|case| case.services.clone())
            .await?
            .into_iter()
            .flat_map(|m| m.into_iter())
            .collect();
        let params: dynconf::Value = self
            .get_actions(event, &|case| Some(case.params.clone()))
            .await?
            .into_iter()
            .try_fold(dynconf::Value::Null, dynconf::Value::merge)?;
        Ok(config::project::EventActions {
            run_pipelines,
            services,
            params,
        })
    }

    pub async fn get_actions<T>(
        &self,
        event: &Event,
        f: &impl Fn(&Trigger) -> Option<T>,
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

    pub async fn create_cron_jobs(
        &self,
        cron_engine: &Cron,
        state: &State,
    ) -> Result<Vec<T>, anyhow::Error> {
        for (_, triggers) in self.actions.iter() {
            for trigger in triggers.iter() {
                if let TriggerType::Cron{project_id, trigger_id, rule} = trigger.on {
                    cron_engine.add_fn(rule, || {
                        let call_context: &CallContext = state.get().unwrap();
                        call_context
                        .call_trigger(&project_id, &trigger_id, dry_run.unwrap_or(false))
                        .await;
                    }).unwrap();
                }
            }  
        }
    }
}

impl TriggerType {
    async fn check_matched(&self, event: &Event) -> bool {
        match self {
            TriggerType::Cron {
                project_id,
                trigger_id,
            } => match event {
                Event::Cron {
                    project_id: event_project_id,
                    trigger_id: event_trigger_id,
                } => project_id == event_project_id && trigger_id == event_trigger_id,
                _ => false,
            },
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
                    }

                    match diffs {
                        config::repo::Diff::Changes {
                            changes,
                            commit_message,
                        } => {
                            for pattern in exclude_commits.iter() {
                                if pattern.is_match(commit_message) {
                                    return false;
                                }
                            }

                            let mut matched = false;

                            for diff in changes.iter() {
                                let mut current_matches = false;

                                for pattern in patterns.iter() {
                                    if pattern.is_match(diff) {
                                        current_matches = true;
                                        break;
                                    }
                                }

                                for pattern in exclude_patterns.iter() {
                                    if pattern.is_match(diff) {
                                        current_matches = false;
                                        break;
                                    }
                                }

                                matched |= current_matches;
                            }

                            matched
                        }
                        config::repo::Diff::Whole => true,
                    }
                }
                _ => false,
            },
        }
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
    pub struct Actions {
        actions: HashMap<String, Vec<Trigger>>,
    }

    #[derive(Deserialize, Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    enum TriggerType {
        #[serde(rename = "call")]
        Call,

        #[serde(rename = "changed")]
        FileChanged,

        #[serde(rename = "cron")]
        Cron,
    }

    #[derive(Deserialize, Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    enum ServiceAction {
        #[serde(rename = "deploy")]
        Deploy,
    }

    #[derive(Deserialize, Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    struct Trigger {
        #[serde(rename = "on")]
        on: TriggerType,
        run_pipelines: Option<Vec<String>>,
        services: Option<HashMap<String, ServiceAction>>,
        repo_id: Option<String>,
        rule: Option<String>,
        changes: Option<Vec<String>>,
        exclude_changes: Option<Vec<String>>,
        exclude_commits: Option<Vec<String>>,
        params: Option<util::DynAny>,
    }

    #[async_trait::async_trait]
    impl util::DynValue for Actions {
        type Target = super::Actions;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            Ok(super::Actions {
                actions: self.actions.load(state).await?,
            })
        }
    }

    #[async_trait::async_trait]
    impl util::DynValue for Trigger {
        type Target = super::Trigger;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            let dynobj = config::utils::get_dyn_object(state)?;
            let project_id = dynobj
                .project
                .ok_or_else(|| anyhow!("No project binding"))?
                .id;
            let trigger_id = dynobj._id.ok_or_else(|| anyhow!("No _id binding"))?;

            let on = match self.on {
                TriggerType::Call => super::TriggerType::Call {
                    project_id,
                    trigger_id,
                },
                TriggerType::Cron => {                    
                    let rule = self
                        .rule
                        .ok_or_else(|| anyhow!("'rule' field required for on: cron"))?; 
                    
                    super::TriggerType::Cron {
                        project_id,
                        trigger_id,
                        rule,
                    }
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
                        .ok_or_else(|| anyhow!("'repo_id' field required for on: changed"))?;
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
                services: self.services.load(state).await?,
                params: self.params.load(state).await?.unwrap_or_default(),
                on,
            })
        }
    }

    #[async_trait::async_trait]
    impl util::DynValue for ServiceAction {
        type Target = super::ServiceAction;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            Ok(match self {
                ServiceAction::Deploy => super::ServiceAction::Deploy,
            })
        }
    }
}
