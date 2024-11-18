use std::path::PathBuf;

use crate::config;

pub struct ServiceConfig {
    pub data_dir: PathBuf,
    pub repos_path: PathBuf,
    pub data_path: PathBuf,
    pub internal_path: PathBuf,
    pub secrets: config::secrets::Secrets,
    pub tokens: config::permissions::Tokens,
    pub projects_store: config::projects::ProjectsStore,
}

impl std::fmt::Debug for ServiceConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceConfig")
            .field("data_dir", &self.data_dir)
            .field("repos_path", &self.repos_path)
            .field("data_path", &self.data_path)
            .field("internal_path", &self.internal_path)
            .field("secrets", &self.secrets)
            .field("tokens", &self.tokens)
            .field("projects_store", &"<dynamic object>")
            .finish()
    }
}

pub enum ActionEvent {
    ConfigReloaded,
    ProjectReloaded {
        project_id: String,
    },
    DirectCall {
        project_id: String,
        trigger_id: String,
    },
    UpdateRepos {
        project_id: String,
        repos: Vec<String>,
    },
}

impl ServiceConfig {
    pub fn check_allowed<S: AsRef<str>>(
        &self,
        token: Option<S>,
        action: config::permissions::ActionType,
    ) -> bool {
        self.tokens.check_allowed(token, action)
    }
}

pub use dyn_obj::DynServiceConfig;

mod dyn_obj {
    use std::path::PathBuf;

    use crate::config;

    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize)]
    pub struct DynServiceConfig {
        pub secrets: Option<config::secrets::DynSecrets>,
        pub internal_path: PathBuf,
        pub repos_path: PathBuf,
        pub data_path: PathBuf,
    }

    impl From<&super::ServiceConfig> for DynServiceConfig {
        fn from(config: &super::ServiceConfig) -> Self {
            Self {
                secrets: Some((&config.secrets).into()),
                internal_path: config.internal_path.clone(),
                repos_path: config.repos_path.clone(),
                data_path: config.data_path.clone(),
            }
        }
    }
}

pub mod raw {
    use crate::config;

    use dynconf::*;
    use serde::{Deserialize, Serialize};

    use anyhow::Result;

    const DEFAULT_REPOS_PATH: &str = "repos";
    const DEFAULT_DATA_PATH: &str = "data";
    const DEFAULT_INTERNAL_PATH: &str = "internal";
    const DEFAULT_DATA_DIR_EXPR: &str = "${~/.uci}";

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct ServiceConfig {
        data_dir: Option<util::DynPath>,
        secrets: Option<util::Dyn<config::secrets::raw::Secrets>>,
        tokens: Option<util::Dyn<config::permissions::raw::Tokens>>,
        projects_store: util::Dyn<ProjectsStore>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum ProjectsStore {
        Static {
            projects: util::Dyn<util::Lazy<config::static_projects::raw::Projects>>,
        },
    }

    #[async_trait::async_trait]
    impl util::DynValue for ServiceConfig {
        type Target = super::ServiceConfig;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            // ${~/.uci} -> <home>/.uci
            let default_data_dir = eval_string(state, DEFAULT_DATA_DIR_EXPR).await?;
            let data_dir = self
                .data_dir
                .load(state)
                .await?
                .unwrap_or_else(|| default_data_dir.into());

            let internal_path = data_dir.join(DEFAULT_INTERNAL_PATH);
            let repos_path = data_dir.join(DEFAULT_REPOS_PATH);
            let data_path = data_dir.join(DEFAULT_DATA_PATH);

            std::fs::create_dir_all(&internal_path)?;
            std::fs::create_dir_all(&repos_path)?;
            std::fs::create_dir_all(&data_path)?;

            state.mutate_global(config::utils::wrap_dyn_f(|mut dynconf| {
                dynconf.config = Some(super::DynServiceConfig {
                    secrets: None,
                    internal_path: internal_path.clone(),
                    repos_path: repos_path.clone(),
                    data_path: data_path.clone(),
                });
                Ok(dynconf)
            }))?;

            let secrets = self.secrets.load(state).await?.unwrap_or_default();

            state.mutate_global(config::utils::wrap_dyn_f(|mut dynconf| {
                dynconf.config.as_mut().unwrap().secrets = Some((&secrets).into());
                Ok(dynconf)
            }))?;

            let tokens = self.tokens.load(state).await?.unwrap_or_default();
            let projects_store = self.projects_store.load(state).await?;

            Ok(super::ServiceConfig {
                data_dir,
                repos_path,
                data_path,
                internal_path,
                secrets,
                tokens,
                projects_store,
            })
        }
    }

    #[async_trait::async_trait]
    impl util::DynValue for ProjectsStore {
        type Target = config::projects::ProjectsStore;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            match self {
                ProjectsStore::Static { projects } => {
                    let projects_store =
                        config::static_projects::StaticProjects::new(projects.load(state).await?)
                            .await?;
                    config::projects::ProjectsStore::with_manager(projects_store).await
                }
            }
        }
    }
}
