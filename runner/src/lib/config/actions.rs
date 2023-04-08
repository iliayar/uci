use crate::lib::git;

use std::collections::{HashMap, HashSet};

use std::path::PathBuf;

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
    ConfigReload,
    ProjectReload {
        project_id: String,
    },
}

pub enum Event {
    ConfigReloaded,
    ProjectReloaded {
        project_id: String,
    },
    Call {
        project_id: String,
        trigger_id: String,
    },
    RepoUpdate(super::ReposDiffs),
}

pub const ACTIONS_CONFIG: &str = "actions.yaml";

impl Actions {
    pub async fn load<'a>(context: &super::LoadContext<'a>) -> Result<Actions, LoadConfigError> {
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
            TriggerType::ConfigReload => {
                matches!(Event::ConfigReloaded, event)
            }
            TriggerType::ProjectReload { project_id } => match event {
                Event::ProjectReloaded {
                    project_id: event_project_id,
                } => project_id == event_project_id,
                _ => false,
            },
        }
    }
}

mod raw {
    use std::collections::HashMap;

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

        #[serde(rename = "config_reload")]
        ConfigReload,

        #[serde(rename = "project_reload")]
        ProjectReload,
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
    }

    impl config::LoadRawSync for Actions {
        type Output = super::Actions;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            Ok(super::Actions {
                actions: self.actions.load_raw(context)?,
            })
        }
    }

    impl config::LoadRawSync for TriggerType {
        type Output = super::TriggerType;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            Ok(match self {
                TriggerType::Call => super::TriggerType::Call {
                    project_id: context.project_id()?.to_string(),
                    trigger_id: context.extra("_id")?.to_string(),
                },
                TriggerType::ProjectReload => super::TriggerType::ProjectReload {
                    project_id: context.project_id()?.to_string(),
                },
                TriggerType::ConfigReload => super::TriggerType::ConfigReload,
            })
        }
    }

    impl config::LoadRawSync for Trigger {
        type Output = super::Trigger;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            if let TriggerType::ConfigReload = self.on {
                if self.reload_config.is_some() {
                    return Err(anyhow!(
                        "Trigger on config_reload is disallowed with reload_config action"
                    )
                    .into());
                }
            }

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

            let project_id = context.project_id()?.to_string();

            Ok(super::Trigger {
                on: self.on.load_raw(context)?,
                run_pipelines: self.run_pipelines,
                services: self.services.load_raw(context)?,
                reload_config: self.reload_config.is_some(),
                reload_project: self.reload_project.is_some(),
            })
        }
    }

    impl config::LoadRawSync for ServiceAction {
        type Output = super::ServiceAction;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            Ok(match self {
                ServiceAction::Deploy => super::ServiceAction::Deploy,
            })
        }
    }

    pub async fn load<'a>(
        context: &config::LoadContext<'a>,
    ) -> Result<super::Actions, super::LoadConfigError> {
        let path = context.project_root()?.join(super::ACTIONS_CONFIG);
        if !path.exists() {
            return Ok(Default::default());
        }
        config::load_sync::<Actions>(path, context).await
    }
}
