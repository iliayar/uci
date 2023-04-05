use crate::lib::git;

use std::collections::{HashMap, HashSet};

use std::path::PathBuf;

use log::*;

use super::LoadConfigError;

#[derive(Debug)]
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
}

impl Actions {
    pub async fn load<'a>(context: &super::LoadContext<'a>) -> Result<Actions, LoadConfigError> {
        raw::load(context).await
    }

    pub fn get(&self, action: &str) -> Option<&Action> {
        self.actions.get(action)
    }
}

impl Action {
    pub async fn get_matched_pipelines(
        &self,
        diffs: &super::ReposDiffs,
    ) -> Result<HashSet<String>, super::ExecutionError> {
        Ok(self
            .get_actions(diffs, |case| case.run_pipelines.clone())
            .await?
            .into_iter()
            .map(|v| v.into_iter())
            .flatten()
            .collect())
    }

    pub async fn get_service_actions(
        &self,
        diffs: &super::ReposDiffs,
    ) -> Result<HashMap<String, super::ServiceAction>, super::ExecutionError> {
        Ok(self
            .get_actions(diffs, |case| case.services.clone())
            .await?
            .into_iter()
            .map(|m| m.into_iter())
            .flatten()
            .collect())
    }

    async fn get_actions<T>(
        &self,
        diffs: &super::ReposDiffs,
        f: impl Fn(&super::Case) -> Option<T>,
    ) -> Result<Vec<T>, super::ExecutionError> {
        let mut res = Vec::new();

        for (i, case) in self.cases.iter().enumerate() {
            if case.condition.check_matched(diffs).await {
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
    async fn check_matched(&self, diffs: &HashMap<String, Vec<String>>) -> bool {
        match self {
            Condition::Always => true,
        }
    }
}

mod raw {
    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};

    use crate::lib::config;

    const ACTIONS_CONFIG: &str = "actions.yaml";

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
        config::load_sync::<Actions>(context.project_root()?.join(ACTIONS_CONFIG), context).await
    }
}
