use std::{collections::HashMap, path::PathBuf};

use anyhow::anyhow;

#[derive(Default, Clone)]
pub struct LoadContext<'a> {
    typed: HashMap<std::any::TypeId, &'a (dyn std::any::Any + Sync)>,
    named: HashMap<String, &'a (dyn std::any::Any + Sync)>,
}

pub mod binding {
    use std::collections::HashMap;

    use crate::lib::config;

    pub struct Params<'a> {
        pub value: &'a HashMap<String, String>,
    }

    pub struct Networks<'a> {
        pub value: &'a HashMap<String, config::Network>,
    }

    pub struct Volumes<'a> {
        pub value: &'a HashMap<String, config::Volume>,
    }
}

impl<'a> LoadContext<'a> {
    pub fn set<T: std::any::Any + Sync>(&mut self, value: &'a T) {
        self.typed.insert(
            std::any::TypeId::of::<T>(),
            value as &(dyn std::any::Any + Sync),
        );
    }
    pub fn get<T: std::any::Any + Sync>(&self) -> Result<&T, super::LoadConfigError> {
        let type_id = std::any::TypeId::of::<T>();
        match self.typed.get(&type_id) {
            Some(v) => match (*v as &dyn std::any::Any).downcast_ref::<T>() {
                Some(v) => Ok(v),
                // By this key there is only one type
                None => unreachable!(),
            },
            None => Err(anyhow!("No type {} in load context", std::any::type_name::<T>()).into()),
        }
    }

    pub fn set_named<T: std::any::Any + Sync, S: AsRef<str>>(&mut self, key: S, value: &'a T) {
        self.named.insert(
            key.as_ref().to_string(),
            value as &(dyn std::any::Any + Sync),
        );
    }
    pub fn get_named<T: std::any::Any, S: AsRef<str>>(
        &self,
        key: S,
    ) -> Result<&T, super::LoadConfigError> {
        let type_id = std::any::TypeId::of::<T>();
        match self.named.get(key.as_ref()) {
            Some(v) => match (*v as &dyn std::any::Any).downcast_ref::<T>() {
                Some(v) => Ok(v),
                None => Err(anyhow!(
                    "Value for {} has type different from {}",
                    key.as_ref(),
                    std::any::type_name::<T>()
                )
                .into()),
            },
            None => Err(anyhow!("No value for {:?} in load context", key.as_ref()).into()),
        }
    }
}

impl<'a> Into<common::vars::Vars> for &LoadContext<'a> {
    fn into(self) -> common::vars::Vars {
        use common::vars::*;
        let mut vars = Vars::default();

        if let Ok(repos) = self.get::<super::Repos>() {
            vars.assign("repos", repos.into()).ok();
        }

        vars
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
                context.set_named("_id", &id);
                let value = value.load_raw(&context)?;
                Ok((id, value))
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl<T: LoadRaw + Send> LoadRaw for HashMap<String, T>
where
    <T as LoadRaw>::Output: Send,
{
    type Output = HashMap<String, <T as LoadRaw>::Output>;

    async fn load_raw(self, context: &LoadContext) -> Result<Self::Output, super::LoadConfigError> {
        let mut res = HashMap::new();
        for (id, value) in self.into_iter() {
            let mut context = context.clone();
            context.set_named("_id", &id);
            let value = value.load_raw(&context).await?;
            res.insert(id, value);
        }
        Ok(res)
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

pub trait AutoLoadRaw {}

impl<T: AutoLoadRaw> LoadRawSync for T {
    type Output = T;

    fn load_raw(self, context: &LoadContext) -> Result<Self::Output, super::LoadConfigError> {
        Ok(self)
    }
}

#[async_trait::async_trait]
impl<T: AutoLoadRaw + Send> LoadRaw for T {
    type Output = T;

    async fn load_raw(self, context: &LoadContext) -> Result<Self::Output, super::LoadConfigError> {
        Ok(self)
    }
}
