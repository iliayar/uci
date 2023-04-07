use crate::lib::git;

use std::collections::{HashMap, HashSet};

use std::path::PathBuf;

use log::*;

use super::LoadConfigError;

#[derive(Debug, Default)]
pub struct Actions {
    actions: HashMap<String, Action>,
}

#[derive(Debug)]
pub struct Action {
    update_repos: Vec<String>,
    cases: Vec<Case>,
}

#[derive(Debug, Clone)]
pub enum ServiceAction {
    Deploy,
}

#[derive(Debug)]
pub struct Case {
    condition: Condition,
    run_pipelines: Option<Vec<String>>,
    services: Option<HashMap<String, ServiceAction>>,
}

#[derive(Debug)]
pub enum Condition {
    Always,
    OnConfigReload,
}

pub enum Trigger {
    ConfigReloaded,
    // NOTE: Dont like it btw
    RepoUpdate(super::ReposDiffs),
}

pub const ACTIONS_CONFIG: &str = "actions.yaml";

impl Actions {
    pub async fn load<'a>(context: &super::LoadContext<'a>) -> Result<Actions, LoadConfigError> {
        raw::load(context).await
    }

    pub fn get(&self, action: &str) -> Option<&Action> {
        self.actions.get(action)
    }

    pub async fn get_matched_pipelines(
        &self,
        trigger: &Trigger,
    ) -> Result<HashSet<String>, super::ExecutionError> {
        Ok(self
            .get_actions(&trigger, &|case| case.run_pipelines.clone())
            .await?
            .into_iter()
            .map(|v| v.into_iter())
            .flatten()
            .collect())
    }

    pub async fn get_service_actions(
        &self,
        trigger: &Trigger,
    ) -> Result<HashMap<String, super::ServiceAction>, super::ExecutionError> {
        Ok(self
            .get_actions(&trigger, &|case| case.services.clone())
            .await?
            .into_iter()
            .map(|m| m.into_iter())
            .flatten()
            .collect())
    }

    pub async fn get_actions<T>(
        &self,
        trigger: &Trigger,
        f: &impl Fn(&super::Case) -> Option<T>,
    ) -> Result<Vec<T>, super::ExecutionError> {
        let mut actions = Vec::new();
        for (action_id, action) in self.actions.iter() {
            actions.append(&mut action.get_actions(trigger, f).await?);
        }
        Ok(actions)
    }
}

impl Action {
    async fn get_actions<T>(
        &self,
        trigger: &Trigger,
        f: &impl Fn(&super::Case) -> Option<T>,
    ) -> Result<Vec<T>, super::ExecutionError> {
        let mut res = Vec::new();

        for (i, case) in self.cases.iter().enumerate() {
            if case.condition.check_matched(trigger).await {
                info!("Match condition {}", i);
                if let Some(value) = f(case) {
                    res.push(value);
                }
            }
        }

        Ok(res)
    }

    pub async fn get_diffs(
        &self,
        config: &super::ServiceConfig,
        repos: &super::Repos,
    ) -> Result<super::ReposDiffs, super::ExecutionError> {
        repos.pull_all(config, &self.update_repos).await
    }
}

impl Condition {
    async fn check_matched(&self, trigger: &Trigger) -> bool {
        match self {
            Condition::Always => {
                !matches!(Trigger::ConfigReloaded, trigger)
	    },
            Condition::OnConfigReload => {
                matches!(Trigger::ConfigReloaded, trigger)
            }
        }
    }
}

mod raw {
    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};

    use crate::lib::config;

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Actions {
        actions: HashMap<String, Action>,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Action {
        update_repos: Option<Vec<String>>,
        conditions: Vec<Condition>,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    enum ConditionType {
        #[serde(rename = "always")]
        Always,

        #[serde(rename = "on_config_reload")]
        OnConfigReload,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    enum ServiceAction {
        #[serde(rename = "deploy")]
        Deploy,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Condition {
        #[serde(rename = "type")]
        t: ConditionType,
        run_pipelines: Option<Vec<String>>,
        services: Option<HashMap<String, ServiceAction>>,
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

    impl config::LoadRawSync for Action {
        type Output = super::Action;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            Ok(super::Action {
                update_repos: self.update_repos.unwrap_or_default(),
                cases: self.conditions.load_raw(context)?,
            })
        }
    }

    impl config::LoadRawSync for ConditionType {
        type Output = super::Condition;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            Ok(match self {
                ConditionType::Always => super::Condition::Always,
                ConditionType::OnConfigReload => super::Condition::OnConfigReload,
            })
        }
    }

    impl config::LoadRawSync for Condition {
        type Output = super::Case;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            Ok(super::Case {
                condition: self.t.load_raw(context)?,
                run_pipelines: self.run_pipelines,
                services: self.services.load_raw(context)?,
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
