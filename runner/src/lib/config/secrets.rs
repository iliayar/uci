use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Default, Clone)]
pub struct Secrets {
    secrets: HashMap<String, String>,
}

impl Secrets {
    pub async fn load(path: PathBuf) -> Result<Secrets, super::LoadConfigError> {
        let content = tokio::fs::read_to_string(path).await?;
        Ok(Secrets {
            secrets: serde_yaml::from_str(&content)?,
        })
    }
}

impl Into<common::vars::Vars> for &Secrets {
    fn into(self) -> common::vars::Vars {
        (&self.secrets).into()
    }
}
