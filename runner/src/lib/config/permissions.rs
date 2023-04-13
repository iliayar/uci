use std::{collections::HashMap, path::PathBuf};

use anyhow::Error;

#[derive(Debug, Default, Clone)]
pub struct Tokens {
    anonymous: Permissions,
    tokens: HashMap<String, Permissions>,
}

#[derive(Debug, Clone)]
pub struct Permissions {
    read: bool,
    write: bool,
    execute: bool,
}

impl Default for Permissions {
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

impl Tokens {
    pub fn check_allowed<S: AsRef<str>>(&self, token: Option<S>, action: ActionType) -> bool {
        if let Some(token) = token {
            self.tokens
                .get(token.as_ref())
                .cloned()
                .unwrap_or_default()
                .check_allowed(action)
        } else {
            self.anonymous.check_allowed(action)
        }
    }
}

impl Permissions {
    pub fn check_allowed(&self, action: ActionType) -> bool {
        match action {
            ActionType::Write => self.write,
            ActionType::Read => self.read,
            ActionType::Execute => self.execute,
        }
    }
}

pub mod permissions_raw {
    pub use super::raw::Tokens;
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::lib::config::load::LoadRawSync;
    use crate::lib::{config, utils};

    use log::*;

    pub type Tokens = Vec<Token>;

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Token {
        token: Option<String>,
        #[serde(default)]
        permissions: Vec<Permissions>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub enum Permissions {
        #[serde(rename = "write")]
        Write,

        #[serde(rename = "read")]
        Read,

        #[serde(rename = "execute")]
        Execute,
    }

    impl config::LoadRawSync for Vec<Permissions> {
        type Output = super::Permissions;

        fn load_raw(
            self,
            context: &config::State,
        ) -> Result<Self::Output, anyhow::Error> {
            let mut result = super::Permissions::default();

            for perm in self.into_iter() {
                match perm {
                    Permissions::Write => {
                        result.write = true;
                    }
                    Permissions::Read => {
                        result.read = true;
                    }
                    Permissions::Execute => {
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
            context: &config::State,
        ) -> Result<Self::Output, anyhow::Error> {
            let mut anon: Option<super::Permissions> = None;
            let mut tokens = HashMap::new();

            let vars: common::vars::Vars = context.into();

            for perm in self.into_iter() {
                if let Some(token) = perm.token {
                    let token = vars.eval(&token)?;
                    tokens.insert(token, perm.permissions.load_raw(context)?);
                } else {
                    if anon.is_some() {
                        warn!("Anonymous permissions mentioned more than one time, skiping it");
                        continue;
                    }
                    anon = Some(perm.permissions.load_raw(context)?);
                }
            }

            Ok(super::Tokens {
                anonymous: anon.unwrap_or_default(),
                tokens,
            })
        }
    }
}
