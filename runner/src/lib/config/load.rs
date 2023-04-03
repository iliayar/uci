use std::path::PathBuf;

use anyhow::anyhow;

#[derive(Default)]
pub struct LoadContext<'a> {
    config: Option<&'a super::ServiceConfig>,
}

impl<'a> LoadContext<'a> {
    pub fn set_config(&mut self, config: &'a super::ServiceConfig) {
        self.config = Some(config);
    }

    pub fn config(&self) -> Result<&super::ServiceConfig, anyhow::Error> {
        self.config
            .clone()
            .ok_or(anyhow!("config is not set in LoadContext"))
    }
}

pub trait LoadRaw
where
    Self: for<'a> serde::Deserialize<'a>,
{
    type Output;
    fn load_raw(self, context: &mut LoadContext) -> Result<Self::Output, super::LoadConfigError>;
}

pub async fn load<'a, T: LoadRaw>(
    path: PathBuf,
    context: &mut LoadContext<'a>,
) -> Result<<T as LoadRaw>::Output, super::LoadConfigError> {
    let content = tokio::fs::read_to_string(path).await?;
    serde_yaml::from_str::<T>(&content)?.load_raw(context)
}
