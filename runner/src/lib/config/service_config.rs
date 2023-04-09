use std::{collections::HashMap, path::PathBuf};

use super::LoadConfigError;

#[derive(Debug)]
pub struct ServiceConfig {
    pub data_dir: PathBuf,
    pub repos_path: PathBuf,
    pub data_path: PathBuf,
    pub internal_path: PathBuf,
    pub worker_url: Option<String>,
    pub secrets: HashMap<String, String>,
    pub tokens: Option<Tokens>,
}

#[derive(Debug)]
pub struct Tokens {
    anonymous: Permissions,
    tokens: HashMap<String, Permissions>,
}

#[derive(Debug, Default, Clone)]
pub struct Permissions {
    global: Permission,
    projects: HashMap<String, Permission>,
}

#[derive(Debug, Clone)]
pub struct Permission {
    read: bool,
    write: bool,
    execute: bool,
}

impl Default for Permission {
    fn default() -> Self {
        Self {
            read: false,
            write: false,
            execute: false,
        }
    }
}

pub enum ActionType {
    Write,
    Read,
    Execute,
}

impl ServiceConfig {
    pub async fn load<'a>(
        context: &super::LoadContext<'a>,
    ) -> Result<ServiceConfig, LoadConfigError> {
        raw::load(context).await
    }

    pub fn check_allowed<S: AsRef<str>, PS: AsRef<str>>(
        &self,
        token: Option<S>,
        project: Option<PS>,
        action: ActionType,
    ) -> bool {
        if let Some(tokens) = self.tokens.as_ref() {
            if let Some(token) = token {
                tokens
                    .tokens
                    .get(token.as_ref())
                    .cloned()
                    .unwrap_or_default()
                    .check_allowed(project.as_ref(), action)
            } else {
                tokens.anonymous.check_allowed(project.as_ref(), action)
            }
        } else {
            true
        }
    }
}

impl Permissions {
    pub fn check_allowed<PS: AsRef<str>>(&self, project: Option<PS>, action: ActionType) -> bool {
        if let Some(project) = project {
            self.projects
                .get(project.as_ref())
                .cloned()
                .unwrap_or_default()
                .check_allowed(action)
        } else {
            self.global.check_allowed(action)
        }
    }
}

impl Permission {
    pub fn check_allowed(&self, action: ActionType) -> bool {
        match action {
            ActionType::Write => self.write,
            ActionType::Read => self.read,
            ActionType::Execute => self.execute,
        }
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::lib::config::load::LoadRawSync;
    use crate::lib::{config, utils};

    use log::*;

    const SERVICE_CONFIG: &str = "conf.yaml";

    const DEFAULT_REPOS_PATH: &str = "repos";
    const DEFAULT_DATA_PATH: &str = "data";
    const DEFAULT_INTERNAL_PATH: &str = "internal";
    const DEFAULT_DATA_DIR: &str = "~/.microci";

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ServiceConfig {
        data_dir: Option<String>,
        worker_url: Option<String>,
        secrets: Option<String>,
        tokens: Option<Vec<Token>>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Token {
        token: Option<String>,
	#[serde(default)]
        global: Vec<Permission>,
	#[serde(default)]
        projects: HashMap<String, Vec<Permission>>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    enum Permission {
        #[serde(rename = "write")]
        Write,

        #[serde(rename = "read")]
        Read,

        #[serde(rename = "execute")]
        Execute,
    }

    impl config::LoadRawSync for Vec<Permission> {
        type Output = super::Permission;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            let mut result = super::Permission::default();

            for perm in self.into_iter() {
                match perm {
                    Permission::Write => {
                        result.write = true;
                    }
                    Permission::Read => {
                        result.read = true;
                    }
                    Permission::Execute => {
                        result.execute = true;
                    }
                }
            }

            Ok(result)
        }
    }

    impl config::LoadRawSync for Vec<Token> {
        type Output = super::Tokens;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            let mut anon: Option<super::Permissions> = None;
            let mut tokens = HashMap::new();

            let vars: common::vars::Vars = context.into();

            for perm in self.into_iter() {
                if let Some(token) = perm.token {
                    let token = vars.eval(&token)?;
                    tokens.insert(
                        token,
                        super::Permissions {
                            global: perm.global.load_raw(context)?,
                            projects: perm.projects.load_raw(context)?,
                        },
                    );
                } else {
                    if anon.is_some() {
                        warn!("Anonymous permissions mentioned more than one time, skiping it");
                        continue;
                    }
                    anon = Some(super::Permissions {
                        global: perm.global.load_raw(context)?,
                        projects: perm.projects.load_raw(context)?,
                    });
                }
            }

            Ok(super::Tokens {
                anonymous: anon.unwrap_or_default(),
                tokens,
            })
        }
    }

    #[async_trait::async_trait]
    impl config::LoadRaw for ServiceConfig {
        type Output = super::ServiceConfig;

        async fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            let data_dir =
                utils::try_expand_home(self.data_dir.unwrap_or(DEFAULT_DATA_DIR.to_string()));
            let secrets = if let Some(secrets) = self.secrets {
                let secrets_path =
                    utils::abs_or_rel_to_dir(secrets, context.configs_root()?.clone());
                Some(load_secrets(secrets_path).await?)
            } else {
                None
            };

            let mut config = super::ServiceConfig {
                data_path: data_dir.join(DEFAULT_DATA_PATH),
                repos_path: data_dir.join(DEFAULT_REPOS_PATH),
                internal_path: data_dir.join(DEFAULT_INTERNAL_PATH),
                worker_url: self.worker_url,
                secrets: secrets.unwrap_or_default(),
                tokens: None,
                data_dir,
            };

            if let Some(tokens) = self.tokens {
                let mut context = context.clone();
                context.set_config(&config);
                config.tokens = Some(tokens.load_raw(&context)?);
            }

            Ok(config)
        }
    }

    async fn load_secrets(
        path: PathBuf,
    ) -> Result<HashMap<String, String>, config::LoadConfigError> {
        let content = tokio::fs::read_to_string(path).await?;
        Ok(serde_yaml::from_str(&content)?)
    }

    pub async fn load<'a>(
        context: &config::LoadContext<'a>,
    ) -> Result<super::ServiceConfig, super::LoadConfigError> {
        let path = context.configs_root()?.join(SERVICE_CONFIG);
        config::load::<ServiceConfig>(path.clone(), context)
            .await
            .map_err(|err| {
                anyhow::anyhow!("Failed to load service_config from {:?}: {}", path, err).into()
            })
    }
}
