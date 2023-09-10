use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct Tokens {
    anonymous: Permissions,
    tokens: HashMap<String, Permissions>,
}

#[derive(Debug, Clone, Default)]
pub struct Permissions {
    read: bool,
    write: bool,
    execute: bool,
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

    pub fn add(&mut self, token: String, perms: Permissions) {
        self.tokens.insert(token, perms);
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

    pub fn superuser() -> Permissions {
        Permissions {
            read: true,
            write: true,
            execute: true,
        }
    }
}

pub mod raw {
    use dynconf::*;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    use anyhow::Result;
    use log::*;

    #[derive(Serialize, Deserialize, Clone)]
    #[serde(deny_unknown_fields)]
    pub struct Token {
        token: Option<util::DynString>,
        permissions: Option<Permissions>,
    }

    #[derive(Serialize, Deserialize, Clone)]
    #[serde(deny_unknown_fields)]
    pub enum Permission {
        #[serde(rename = "write")]
        Write,

        #[serde(rename = "read")]
        Read,

        #[serde(rename = "execute")]
        Execute,
    }

    #[derive(Serialize, Deserialize, Clone)]
    #[serde(transparent)]
    pub struct Permissions {
        permissions: Vec<Permission>,
    }

    #[derive(Serialize, Deserialize, Clone)]
    #[serde(transparent)]
    pub struct Tokens {
        tokens: Vec<Token>,
    }

    #[async_trait::async_trait]
    impl util::DynValue for Permissions {
        type Target = super::Permissions;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            let mut result = super::Permissions::default();

            for perm in self.permissions.into_iter() {
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

    #[async_trait::async_trait]
    impl util::DynValue for Tokens {
        type Target = super::Tokens;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            let mut anon: Option<super::Permissions> = None;
            let mut tokens = HashMap::new();

            for perm in self.tokens.into_iter() {
                if let Some(token) = perm.token {
                    tokens.insert(
                        token.load(state).await?,
                        perm.permissions.load(state).await?.unwrap_or_default(),
                    );
                } else {
                    if anon.is_some() {
                        warn!("Anonymous permissions mentioned more than one time, skiping it");
                        continue;
                    }
                    anon = Some(perm.permissions.load(state).await?.unwrap_or_default());
                }
            }

            Ok(super::Tokens {
                anonymous: anon.unwrap_or_default(),
                tokens,
            })
        }
    }
}
