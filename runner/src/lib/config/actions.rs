use crate::lib::git;

use std::collections::HashMap;

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

#[derive(Debug)]
pub struct Case {
    condition: Condition,
    run_pipelines: Vec<String>,
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
        config: &super::ServiceConfig,
        repos: &super::Repos,
    ) -> Result<Vec<String>, super::ExecutionError> {
        info!("Pulling repos");

        let mut repo_diffs = HashMap::new();
        for repo_id in self.update_repos.iter() {
            repo_diffs.insert(repo_id.clone(), repos.pull(config, repo_id).await?);
        }

        let mut run_pipelines = Vec::new();
        for (i, case) in self.cases.iter().enumerate() {
            if case.condition.check_matched(&repo_diffs).await {
                info!("Match condition {}", i);
                run_pipelines.append(&mut case.run_pipelines.clone());
            }
        }

        Ok(run_pipelines)
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
        actions: Vec<Action>,
    }

    #[derive(Deserialize, Serialize)]
    struct Action {
        id: String,
        update_repos: Vec<String>,
        conditions: Vec<Condition>,
    }

    #[derive(Deserialize, Serialize)]
    enum ConditionType {
        #[serde(rename = "always")]
        Always,
    }

    #[derive(Deserialize, Serialize)]
    struct Condition {
        #[serde(rename = "type")]
        t: ConditionType,
        run_pipelines: Vec<String>,
    }

    impl TryFrom<Actions> for super::Actions {
        type Error = super::LoadConfigError;

        fn try_from(value: Actions) -> Result<Self, Self::Error> {
            let mut actions = HashMap::new();

            for Action {
                id,
                update_repos,
                conditions,
            } in value.actions.into_iter()
            {
                let cases = conditions
                    .into_iter()
                    .map(|Condition { t, run_pipelines }| {
                        let condition = match t {
                            ConditionType::Always => super::Condition::Always,
                        };

                        super::Case {
                            condition,
                            run_pipelines,
                        }
                    })
                    .collect();

                actions.insert(
                    id,
                    super::Action {
                        update_repos,
                        cases,
                    },
                );
            }

            Ok(super::Actions { actions })
        }
    }

    pub async fn parse(path: PathBuf) -> Result<super::Actions, super::LoadConfigError> {
        config::utils::load_file::<Actions, _>(path).await
    }
}
