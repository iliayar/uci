use std::{collections::HashMap, path::PathBuf};

use anyhow::anyhow;

#[derive(Default, Clone)]
pub struct LoadContext<'a> {
    configs_root: Option<&'a PathBuf>,
    config: Option<&'a super::ServiceConfig>,
    project_id: Option<&'a str>,
    project_root: Option<&'a PathBuf>,
    project_config: Option<&'a PathBuf>,
    service_id: Option<&'a str>,
    networks: Option<&'a HashMap<String, super::Network>>,
    volumes: Option<&'a HashMap<String, super::Volume>>,

    repos: Option<&'a super::Repos>,

    extra: HashMap<String, &'a str>,
}

impl<'a> LoadContext<'a> {
    pub fn set_config(&mut self, config: &'a super::ServiceConfig) {
        self.config = Some(config);
    }

    pub fn config(&self) -> Result<&super::ServiceConfig, super::LoadConfigError> {
        getter_impl(self.config, "config")
    }

    pub fn set_configs_root(&mut self, configs_root: &'a PathBuf) {
        self.configs_root = Some(configs_root);
    }

    pub fn configs_root(&self) -> Result<&PathBuf, super::LoadConfigError> {
        getter_impl(self.configs_root, "configs_root")
    }

    pub fn set_project_id(&mut self, project_id: &'a str) {
        self.project_id = Some(project_id);
    }

    pub fn project_id(&self) -> Result<&str, super::LoadConfigError> {
        getter_impl(self.project_id, "project_id")
    }

    pub fn set_project_root(&mut self, project_root: &'a PathBuf) {
        self.project_root = Some(project_root);
    }

    pub fn project_root(&self) -> Result<&PathBuf, super::LoadConfigError> {
        getter_impl(self.project_root, "project_root")
    }

    pub fn set_project_config(&mut self, project_config: &'a PathBuf) {
        self.project_config = Some(project_config);
    }

    pub fn project_config(&self) -> Result<&PathBuf, super::LoadConfigError> {
        getter_impl(self.project_config, "project_config")
    }

    pub fn set_service_id(&mut self, service_id: &'a str) {
        self.service_id = Some(service_id);
    }

    pub fn service_id(&self) -> Result<&str, super::LoadConfigError> {
        getter_impl(self.service_id, "service_id")
    }

    pub fn set_networks(&mut self, networks: &'a HashMap<String, super::Network>) {
        self.networks = Some(networks);
    }

    pub fn networks(&self) -> Result<&HashMap<String, super::Network>, super::LoadConfigError> {
        getter_impl(self.networks, "networks")
    }

    pub fn set_volumes(&mut self, volumes: &'a HashMap<String, super::Volume>) {
        self.volumes = Some(volumes);
    }

    pub fn volumes(&self) -> Result<&HashMap<String, super::Volume>, super::LoadConfigError> {
        getter_impl(self.volumes, "volumes")
    }

    pub fn set_repos(&mut self, repos: &'a super::Repos) {
        self.repos = Some(repos);
    }

    pub fn repos(&self) -> Result<&super::Repos, super::LoadConfigError> {
        getter_impl(self.repos, "repos")
    }

    pub fn set_extra(&mut self, key: String, value: &'a str) {
        self.extra.insert(key, value);
    }

    pub fn extra(&self, key: &str) -> Result<&str, super::LoadConfigError> {
        self.extra
            .get(key)
            .map(|k| *k)
            .ok_or(anyhow!("No such extra key: {}", key))
            .map_err(Into::into)
    }
}

impl<'a> Into<common::vars::Vars> for &LoadContext<'a> {
    fn into(self) -> common::vars::Vars {
        use common::vars::*;
        let mut value: HashMap<String, common::vars::Vars> = HashMap::new();

        if let Some(repos) = self.repos {
            value.insert(String::from("repos"), repos.into());
        }

        if let Some(config) = self.config {
            if let Some(project_id) = self.project_id {
                let data_path = config.data_path.join(project_id);
                value.insert(
                    String::from("project"),
                    Value::Object(HashMap::from_iter([(
                        String::from("data"),
                        Value::Object(HashMap::from_iter([(
                            String::from("path"),
                            Value::<()>::String(data_path.to_string_lossy().to_string()),
                        )])),
                    )]))
                    .into(),
                );
            }
        }

        value.into()
    }
}

impl<'a> Into<common::vars::Vars> for LoadContext<'a> {
    fn into(self) -> common::vars::Vars {
        (&self).into()
    }
}

fn getter_impl<'a, T: ?Sized>(
    binding: Option<&'a T>,
    name: &str,
) -> Result<&'a T, super::LoadConfigError> {
    binding
        .ok_or(anyhow!("{} is not set in LoadContext", name))
        .map_err(Into::into)
}

#[async_trait::async_trait]
pub trait LoadRaw {
    type Output;

    async fn load_raw(self, context: &LoadContext) -> Result<Self::Output, super::LoadConfigError>;
}

pub trait LoadRawSync {
    type Output;

    fn load_raw(self, context: &LoadContext) -> Result<Self::Output, super::LoadConfigError>;
}

pub async fn load<'a, T: LoadRaw>(
    path: PathBuf,
    context: &LoadContext<'a>,
) -> Result<<T as LoadRaw>::Output, super::LoadConfigError>
where
    T: for<'b> serde::Deserialize<'b>,
{
    let content = tokio::fs::read_to_string(path).await?;
    serde_yaml::from_str::<T>(&content)?.load_raw(context).await
}

pub async fn load_sync<'a, T: LoadRawSync>(
    path: PathBuf,
    context: &LoadContext<'a>,
) -> Result<<T as LoadRawSync>::Output, super::LoadConfigError>
where
    T: for<'b> serde::Deserialize<'b>,
{
    let content = tokio::fs::read_to_string(path).await?;
    serde_yaml::from_str::<T>(&content)?.load_raw(context)
}

impl<T: LoadRawSync> LoadRawSync for Vec<T> {
    type Output = Vec<<T as LoadRawSync>::Output>;

    fn load_raw(self, context: &LoadContext) -> Result<Self::Output, super::LoadConfigError> {
        self.into_iter().map(|v| Ok(v.load_raw(context)?)).collect()
    }
}

impl<T: LoadRawSync> LoadRawSync for HashMap<String, T> {
    type Output = HashMap<String, <T as LoadRawSync>::Output>;

    fn load_raw(self, context: &LoadContext) -> Result<Self::Output, super::LoadConfigError> {
        self.into_iter()
            .map(|(id, value)| {
                let mut context = context.clone();
                context.set_extra(String::from("_id"), &id);
                let value = value.load_raw(&context)?;
                Ok((id, value))
            })
            .collect()
    }
}

impl<T: LoadRawSync> LoadRawSync for Option<T> {
    type Output = Option<<T as LoadRawSync>::Output>;

    fn load_raw(self, context: &LoadContext) -> Result<Self::Output, super::LoadConfigError> {
        if let Some(value) = self {
            Ok(Some(value.load_raw(context)?))
        } else {
            Ok(None)
        }
    }
}
