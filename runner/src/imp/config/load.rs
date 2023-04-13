use std::{collections::HashMap, path::PathBuf};

use anyhow::anyhow;

#[derive(Default, Clone)]
pub struct State<'a> {
    typed: HashMap<std::any::TypeId, &'a (dyn std::any::Any + Sync)>,
    named: HashMap<String, &'a (dyn std::any::Any + Sync)>,
}

pub mod binding {
    use std::collections::HashMap;

    pub struct Params<'a> {
        pub value: &'a HashMap<String, String>,
    }
}

impl<'a> State<'a> {
    pub fn set<T: std::any::Any + Sync>(&mut self, value: &'a T) {
        self.typed.insert(
            std::any::TypeId::of::<T>(),
            value as &(dyn std::any::Any + Sync),
        );
    }
    pub fn get<T: std::any::Any + Sync>(&self) -> Result<&T, anyhow::Error> {
        let type_id = std::any::TypeId::of::<T>();
        match self.typed.get(&type_id) {
            Some(v) => match (*v as &dyn std::any::Any).downcast_ref::<T>() {
                Some(v) => Ok(v),
                // By this key there is only one type
                None => unreachable!(),
            },
            None => Err(anyhow!(
                "No type {} in load context",
                std::any::type_name::<T>()
            )),
        }
    }

    pub fn set_named<T: std::any::Any + Sync, S: AsRef<str>>(&mut self, key: S, value: &'a T) {
        self.named.insert(
            key.as_ref().to_string(),
            value as &(dyn std::any::Any + Sync),
        );
    }
    pub fn get_named<T: std::any::Any, S: AsRef<str>>(&self, key: S) -> Result<&T, anyhow::Error> {
        let type_id = std::any::TypeId::of::<T>();
        match self.named.get(key.as_ref()) {
            Some(v) => match (*v as &dyn std::any::Any).downcast_ref::<T>() {
                Some(v) => Ok(v),
                None => Err(anyhow!(
                    "Value for {} has type different from {}",
                    key.as_ref(),
                    std::any::type_name::<T>()
                )),
            },
            None => Err(anyhow!("No value for {:?} in load context", key.as_ref())),
        }
    }
}

impl<'a> From<&State<'a>> for common::vars::Vars {
    fn from(val: &State<'a>) -> Self {
        use common::vars::*;
        let mut vars = Vars::default();

        if let Ok(project_info) = val.get::<super::ProjectInfo>() {
            vars.assign("project", project_info.into()).ok();
        }

        if let Ok(config) = val.get::<super::ServiceConfig>() {
            vars.assign("config", config.into()).ok();
        }

        if let Ok(static_projects) = val.get::<super::StaticProjects>() {
            vars.assign("static_projects", static_projects.into()).ok();
        }

        if let Ok(project_params) = val.get_named::<HashMap<String, String>, _>("project_params") {
            vars.assign("params", project_params.into()).ok();
        }

        vars
    }
}

impl<'a> From<State<'a>> for common::vars::Vars {
    fn from(val: State<'a>) -> Self {
        (&val).into()
    }
}

#[async_trait::async_trait]
pub trait LoadRaw {
    type Output;

    async fn load_raw(self, context: &State) -> Result<Self::Output, anyhow::Error>;
}

pub trait LoadRawSync {
    type Output;

    fn load_raw(self, context: &State) -> Result<Self::Output, anyhow::Error>;
}

pub async fn load<'a, T: LoadRaw>(
    path: PathBuf,
    context: &State<'a>,
) -> Result<<T as LoadRaw>::Output, anyhow::Error>
where
    T: for<'b> serde::Deserialize<'b>,
{
    let content = tokio::fs::read_to_string(path).await?;
    serde_yaml::from_str::<T>(&content)?.load_raw(context).await
}

pub async fn load_sync<'a, T: LoadRawSync>(
    path: PathBuf,
    context: &State<'a>,
) -> Result<<T as LoadRawSync>::Output, anyhow::Error>
where
    T: for<'b> serde::Deserialize<'b>,
{
    let content = tokio::fs::read_to_string(path).await?;
    serde_yaml::from_str::<T>(&content)?.load_raw(context)
}

impl<T: LoadRawSync> LoadRawSync for Vec<T> {
    type Output = Vec<<T as LoadRawSync>::Output>;

    fn load_raw(self, context: &State) -> Result<Self::Output, anyhow::Error> {
        self.into_iter().map(|v| v.load_raw(context)).collect()
    }
}

impl<T: LoadRawSync> LoadRawSync for HashMap<String, T> {
    type Output = HashMap<String, <T as LoadRawSync>::Output>;

    fn load_raw(self, context: &State) -> Result<Self::Output, anyhow::Error> {
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

    async fn load_raw(self, context: &State) -> Result<Self::Output, anyhow::Error> {
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

    fn load_raw(self, context: &State) -> Result<Self::Output, anyhow::Error> {
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

    fn load_raw(self, context: &State) -> Result<Self::Output, anyhow::Error> {
        Ok(self)
    }
}

#[async_trait::async_trait]
impl<T: AutoLoadRaw + Send> LoadRaw for T {
    type Output = T;

    async fn load_raw(self, context: &State) -> Result<Self::Output, anyhow::Error> {
        Ok(self)
    }
}
