use std::collections::HashMap;

use anyhow::{anyhow, Result};

#[derive(Debug, Default, Clone)]
pub struct Secrets {
    secrets: HashMap<String, String>,
}

impl Secrets {
    pub fn merge(self, other: Secrets) -> Result<Secrets> {
        let mut secrets = HashMap::new();
        for (id, value) in self.secrets.into_iter().chain(other.secrets.into_iter()) {
            if secrets.contains_key(&id) {
                return Err(anyhow!("Secret {} duplicates", id));
            }
            secrets.insert(id, value);
        }
        Ok(Secrets { secrets })
    }

    pub fn get(&self, k: impl AsRef<str>) -> Option<String> {
        self.secrets.get(k.as_ref()).cloned()
    }

    pub fn merged(self, other: Secrets) -> Secrets {
        Secrets {
            secrets: self
                .secrets
                .into_iter()
                .chain(other.secrets.into_iter())
                .collect(),
        }
    }
}

pub use dyn_obj::DynSecrets;

mod dyn_obj {
    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize)]
    #[serde(transparent)]
    pub struct DynSecrets {
        values: HashMap<String, String>,
    }

    impl From<&super::Secrets> for DynSecrets {
        fn from(secrets: &super::Secrets) -> Self {
            Self {
                values: secrets.secrets.clone(),
            }
        }
    }
}

pub mod raw {
    use dynconf::*;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    use anyhow::Result;

    #[derive(Serialize, Deserialize, Clone)]
    #[serde(transparent)]
    pub struct Secrets {
        secrets: HashMap<String, util::DynString>,
    }

    #[async_trait::async_trait]
    impl util::DynValue for Secrets {
        type Target = super::Secrets;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            Ok(super::Secrets {
                secrets: self.secrets.load(state).await?,
            })
        }
    }
}
