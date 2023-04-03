use crate::lib::git;

use std::collections::{HashMap, HashSet};

use std::path::PathBuf;

use log::*;

use super::LoadConfigError;

const ACTIONS_CONFIG: &str = "actions.yaml";

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
    pub async fn load(project_root: PathBuf) -> Result<Actions, LoadConfigError> {
        raw::parse(project_root.join(ACTIONS_CONFIG)).await
    }

    pub fn get(&self, action: &str) -> Option<&Action> {
        self.actions.get(action)
    }

    // pub async fn check_matched(&self, diffs)
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
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::lib::config;

    #[derive(Deserialize, Serialize)]
    struct Actions {
        actions: HashMap<String, Action>,
    }

    #[derive(Deserialize, Serialize)]
    struct Action {
        update_repos: Option<Vec<String>>,
        conditions: Vec<Condition>,
    }

    #[derive(Deserialize, Serialize)]
    enum ConditionType {
        #[serde(rename = "always")]
        Always,
    }

    #[derive(Deserialize, Serialize)]
    enum ServiceAction {
        #[serde(rename = "deploy")]
        Deploy,
    }

    #[derive(Deserialize, Serialize)]
    struct Condition {
        #[serde(rename = "type")]
        t: ConditionType,
        run_pipelines: Option<Vec<String>>,
        services: Option<HashMap<String, ServiceAction>>,
    }

    impl TryFrom<Actions> for super::Actions {
        type Error = super::LoadConfigError;

        fn try_from(value: Actions) -> Result<Self, Self::Error> {
            let mut actions = HashMap::new();

            for (
                id,
                Action {
                    update_repos,
                    conditions,
                },
            ) in value.actions.into_iter()
            {
                let cases: Result<Vec<_>, super::LoadConfigError> = conditions
                    .into_iter()
                    .map(
                        |Condition {
                             t,
                             run_pipelines,
                             services,
                         }| {
                            let condition = match t {
                                ConditionType::Always => super::Condition::Always,
                            };

                            let services: Option<Result<HashMap<_, _>, super::LoadConfigError>> =
                                services.map(|m| {
                                    m.into_iter()
                                        .map(|(k, v)| Ok((k, super::ServiceAction::try_from(v)?)))
                                        .collect()
                                });
                            let services = if let Some(services) = services {
                                Some(services?)
                            } else {
                                None
                            };

                            Ok(super::Case {
                                condition,
                                run_pipelines,
                                services,
                            })
                        },
                    )
                    .collect();
                let cases = cases?;

                actions.insert(
                    id,
                    super::Action {
                        update_repos: update_repos.unwrap_or(Vec::new()),
                        cases,
                    },
                );
            }

            Ok(super::Actions { actions })
        }
    }

    impl TryFrom<ServiceAction> for super::ServiceAction {
        type Error = super::LoadConfigError;

        fn try_from(value: ServiceAction) -> Result<Self, Self::Error> {
            Ok(match value {
                ServiceAction::Deploy => super::ServiceAction::Deploy,
            })
        }
    }

    pub async fn parse(path: PathBuf) -> Result<super::Actions, super::LoadConfigError> {
        config::utils::load_file::<Actions, _>(path).await
    }
}
