use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Default, Clone)]
pub struct Secrets {
    secrets: HashMap<String, String>,
}

impl Secrets {
    pub async fn load(path: PathBuf) -> Result<Secrets, anyhow::Error> {
        let content = tokio::fs::read_to_string(path).await?;
        Ok(Secrets {
            secrets: serde_yaml::from_str(&content)?,
        })
    }

    pub fn get(&self, k: impl AsRef<str>) -> Option<String> {
        self.secrets.get(k.as_ref()).cloned()
    }
}

impl From<&Secrets> for common::vars::Vars {
    fn from(val: &Secrets) -> Self {
        (&val.secrets).into()
    }
}
