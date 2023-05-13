use std::{collections::HashMap, path::PathBuf};

use anyhow::anyhow;

#[derive(Debug, Default, Clone)]
pub struct Secrets {
    secrets: HashMap<String, String>,
}

pub mod raw {
    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct Secrets {
        secrets: HashMap<String, String>,
    }

    impl crate::config::LoadRawSync for Secrets {
        type Output = super::Secrets;

        fn load_raw(self, state: &common::state::State) -> Result<Self::Output, anyhow::Error> {
            Ok(super::Secrets {
                secrets: self.secrets,
            })
        }
    }
}

impl Secrets {
    pub async fn load(path: PathBuf) -> Result<Secrets, anyhow::Error> {
        let content = tokio::fs::read_to_string(path).await?;
        Ok(Secrets {
            secrets: serde_yaml::from_str(&content)?,
        })
    }

    pub fn merge(self, other: Secrets) -> Result<Secrets, anyhow::Error> {
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
}

impl From<&Secrets> for common::vars::Value {
    fn from(val: &Secrets) -> Self {
        (&val.secrets).into()
    }
}

impl Secrets {
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
