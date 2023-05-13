use std::{collections::HashMap, marker::PhantomData, path::PathBuf};

use common::state::State;

use anyhow::anyhow;
use serde::{Deserialize, Serialize};

use super::utils::{eval_expr, substitute_vars};

pub mod binding {
    use std::collections::HashMap;

    pub struct Params<'a> {
        pub value: &'a HashMap<String, String>,
    }
}

#[async_trait::async_trait]
pub trait LoadRaw {
    type Output;

    async fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error>;
}

pub trait LoadRawSync {
    type Output;

    fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error>;
}

pub async fn load<'a, T: LoadRaw>(
    path: PathBuf,
    state: &State<'a>,
) -> Result<<T as LoadRaw>::Output, anyhow::Error>
where
    T: for<'b> serde::Deserialize<'b>,
{
    let content = tokio::fs::read_to_string(path).await?;
    serde_yaml::from_str::<T>(&content)?.load_raw(state).await
}

pub async fn load_sync<'a, T: LoadRawSync>(
    path: PathBuf,
    state: &State<'a>,
) -> Result<<T as LoadRawSync>::Output, anyhow::Error>
where
    T: for<'b> serde::Deserialize<'b>,
{
    let content = tokio::fs::read_to_string(path).await?;
    serde_yaml::from_str::<T>(&content)?.load_raw(state)
}

impl<T: LoadRawSync> LoadRawSync for Vec<T> {
    type Output = Vec<<T as LoadRawSync>::Output>;

    fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
        self.into_iter().map(|v| v.load_raw(state)).collect()
    }
}

#[async_trait::async_trait]
impl<T: LoadRaw + Send> LoadRaw for Vec<T>
where
    T::Output: Send,
{
    type Output = Vec<T::Output>;

    async fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
        let mut res = Vec::new();
        for v in self {
            res.push(v.load_raw(state).await?);
        }
        Ok(res)
    }
}

impl<T: LoadRawSync> LoadRawSync for HashMap<String, T> {
    type Output = HashMap<String, <T as LoadRawSync>::Output>;

    fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
        self.into_iter()
            .map(|(id, value)| {
                let mut state = state.clone();
                state.set_named("_id", &id);
                let value = value.load_raw(&state)?;
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

    async fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
        let mut res = HashMap::new();
        for (id, value) in self.into_iter() {
            let mut state = state.clone();
            state.set_named("_id", &id);
            let value = value.load_raw(&state).await?;
            res.insert(id, value);
        }
        Ok(res)
    }
}

impl<T: LoadRawSync> LoadRawSync for Option<T> {
    type Output = Option<<T as LoadRawSync>::Output>;

    fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
        if let Some(value) = self {
            Ok(Some(value.load_raw(state)?))
        } else {
            Ok(None)
        }
    }
}

#[async_trait::async_trait]
impl<T: LoadRaw + Send> LoadRaw for Option<T> {
    type Output = Option<<T as LoadRaw>::Output>;

    async fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
        if let Some(value) = self {
            Ok(Some(value.load_raw(state).await?))
        } else {
            Ok(None)
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(transparent)]
pub struct Expr<T> {
    value: common::vars::Value,

    #[serde(skip)]
    _phantom: PhantomData<T>,
}

impl LoadRawSync for Expr<String> {
    type Output = String;

    fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
        match self.value {
            common::vars::Value::Object(_) => Err(anyhow!("Cannot convert object to string")),
            common::vars::Value::List(_) => Err(anyhow!("Cannot convert list to string")),
            common::vars::Value::String(s) => Ok(substitute_vars(state, s)?),
            common::vars::Value::Bool(b) => Ok(b.to_string()),
            common::vars::Value::Integer(i) => Ok(i.to_string()),
            common::vars::Value::None => Ok("none".to_string()),
        }
    }
}

impl LoadRawSync for Expr<bool> {
    type Output = bool;

    fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
        let mut value = self.value;

        if let common::vars::Value::String(s) = value {
            value = eval_expr(state, s)?;
        }

        match value {
            common::vars::Value::Object(_) => Err(anyhow!("Cannot convert object to bool")),
            common::vars::Value::List(_) => Err(anyhow!("Cannot convert list to bool")),
            common::vars::Value::Bool(b) => Ok(b),
            common::vars::Value::String(s) => Ok(s
                .parse()
                .map_err(|err| anyhow!("Failed to parse bool: {}", err))?),
            common::vars::Value::Integer(i) => Ok(i != 0),
            common::vars::Value::None => Ok(false),
        }
    }
}

impl LoadRawSync for Expr<i64> {
    type Output = i64;

    fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
        let mut value = self.value;

        if let common::vars::Value::String(s) = value {
            value = eval_expr(state, s)?;
        }

        match value {
            common::vars::Value::Object(_) => Err(anyhow!("Cannot convert object to integer")),
            common::vars::Value::List(_) => Err(anyhow!("Cannot convert list to integer")),
            common::vars::Value::Bool(b) => Ok(if b { 1 } else { 0 }),
            common::vars::Value::String(s) => Ok(s
                .parse()
                .map_err(|err| anyhow!("Failed to parse integer: {}", err))?),
            common::vars::Value::Integer(i) => Ok(i),
            common::vars::Value::None => Err(anyhow!("Cannot convert none to integer")),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(transparent)]
pub struct AbsPath {
    value: String,
}

impl LoadRawSync for AbsPath {
    type Output = PathBuf;

    fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
        crate::utils::eval_abs_path(state, self.value)
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

impl<T: LoadRawSync> LoadRawSync for OneOrMany<T> {
    type Output = Vec<T::Output>;

    fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
        match self {
            OneOrMany::One(value) => Ok(vec![value.load_raw(state)?]),
            OneOrMany::Many(value) => Ok(value.load_raw(state)?),
        }
    }
}

#[async_trait::async_trait]
impl<T: LoadRaw + Send> LoadRaw for OneOrMany<T>
where
    T::Output: Send,
{
    type Output = Vec<T::Output>;

    async fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
        match self {
            OneOrMany::One(value) => Ok(vec![value.load_raw(state).await?]),
            OneOrMany::Many(values) => Ok(values.load_raw(state).await?),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(transparent)]
pub struct File<T> {
    value: AbsPath,

    _phantom: PhantomData<T>,
}

#[async_trait::async_trait]
impl<T: LoadRawSync + Send> LoadRaw for File<T>
where
    T: for<'a> Deserialize<'a>,
{
    type Output = T::Output;

    async fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
        let path: PathBuf = self.value.load_raw(state)?;
        let content = tokio::fs::read_to_string(path).await?;
        let value: T = serde_yaml::from_str(&content)?;
        Ok(value.load_raw(state)?)
    }
}
