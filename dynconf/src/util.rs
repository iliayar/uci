use futures::{future::BoxFuture, FutureExt};
use serde::{Deserialize, Serialize};

use crate::{eval_expr, eval_string, prelude::*};

use std::{collections::HashMap, marker::PhantomData, path::PathBuf};

#[async_trait::async_trait]
pub trait DynValue {
    type Target;

    async fn load(self, state: &mut State) -> Result<Self::Target>;
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(transparent)]
pub struct Expr<T>
where
    T: for<'a> Deserialize<'a>,
{
    value: String,

    #[serde(skip)]
    _phantom: PhantomData<T>,
}

#[async_trait::async_trait]
impl<T: for<'a> Deserialize<'a> + Send> Evaluate for Expr<T> {
    async fn eval(self, state: &mut State) -> Result<Value> {
        eval_expr(state, &self.value).await
    }
}

#[async_trait::async_trait]
impl<T: Send> DynValue for Expr<T>
where
    T: for<'a> Deserialize<'a>,
    T: DynValue,
{
    type Target = T::Target;

    async fn load(self, state: &mut State) -> Result<Self::Target> {
        let mut state = state.scope();

        let value = Evaluate::eval(self, &mut state).await?;

        Ok(serde_json::from_value::<T>(value.to_json())
            .map_err(|err| {
                anyhow!(
                    "While loading Expr of {}: {err}",
                    std::any::type_name::<T>()
                )
            })?
            .load(&mut state)
            .await?)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(transparent)]
pub struct DynString {
    value: String,
}

#[async_trait::async_trait]
impl Evaluate for DynString {
    async fn eval(self, state: &mut State) -> Result<Value> {
        Ok(Value::String(eval_string(state, &self.value).await?))
    }
}

#[async_trait::async_trait]
impl DynValue for DynString {
    type Target = String;

    async fn load(self, state: &mut State) -> Result<Self::Target> {
        let mut state = state.scope();
        eval_string(&mut state, &self.value).await
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(transparent)]
pub struct DynPath {
    value: String,
}

#[async_trait::async_trait]
impl DynValue for DynPath {
    type Target = PathBuf;

    async fn load(self, state: &mut State) -> Result<Self::Target> {
        let mut state = state.scope();
        Ok(eval_string(&mut state, &self.value).await?.into())
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(transparent)]
pub struct Dyn<T> {
    value: serde_yaml::Value,

    #[serde(skip)]
    _phantom: PhantomData<T>,
}

#[async_trait::async_trait]
impl<T: Send> DynValue for Dyn<T>
where
    T: DynValue,
    T: for<'a> Deserialize<'a>,
{
    type Target = T::Target;

    async fn load(self, state: &mut State) -> Result<Self::Target> {
        let mut state = state.scope();

        match self.value {
            serde_yaml::Value::String(expr) => {
                serde_json::from_value::<T>(eval_expr(&mut state, &expr).await?.to_json())
                    .map_err(|err| {
                        anyhow!(
                            "While loading Dyn expression of {}: {err}",
                            std::any::type_name::<T>()
                        )
                    })?
                    .load(&mut state)
                    .await
            }
            value => {
                serde_yaml::from_value::<T>(value)
                    .map_err(|err| {
                        anyhow!(
                            "While loading Dyn exact value of {}: {err}",
                            std::any::type_name::<T>()
                        )
                    })?
                    .load(&mut state)
                    .await
            }
        }
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, Clone)]
#[serde(transparent)]
pub struct Lazy<T> {
    value: serde_yaml::Value,

    #[serde(skip)]
    _phantom: PhantomData<T>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct LoadedLazy<T> {
    pub(crate) value: serde_yaml::Value,
    pub(crate) current_dir: Option<PathBuf>,

    #[serde(skip)]
    pub(crate) _phantom: PhantomData<T>,
}

#[async_trait::async_trait]
impl<T> DynValue for Lazy<T>
where
    T: Send,
{
    type Target = LoadedLazy<T>;

    async fn load(self, state: &mut State) -> Result<Self::Target> {
        Ok(LoadedLazy {
            value: self.value,
            current_dir: state.get_current_dir(),
            _phantom: PhantomData::default(),
        })
    }
}

#[async_trait::async_trait]
impl<T: DynValue> DynValue for LoadedLazy<T>
where
    T: Send,
    T: for<'a> Deserialize<'a>,
{
    type Target = T::Target;

    async fn load(self, state: &mut State) -> Result<Self::Target> {
        let mut state = state.scope();
        if let Some(current_dir) = self.current_dir {
            state.set_current_dir(current_dir);
        }

        let value = serde_yaml::from_value::<T>(self.value).map_err(|err| {
            anyhow!(
                "While parsing lazy value of {}: {err}",
                std::any::type_name::<T>()
            )
        })?;

        value.load(&mut state).await.map_err(|err| {
            anyhow!(
                "While loading lazy value of {}: {err}",
                std::any::type_name::<T>()
            )
        })
    }
}

pub async fn load<'a, T>(state: &mut State<'a>, file: PathBuf) -> Result<T::Target>
where
    T: for<'b> Deserialize<'b> + DynValue,
{
    let filename: String = file
        .file_name()
        .ok_or_else(|| anyhow!("Cannot load file by path, because it has no name"))?
        .to_string_lossy()
        .to_string();
    state.set_current_dir(
        file.parent()
            .ok_or_else(|| anyhow!("Cannot load file by path, because it has not parent dir"))?
            .to_path_buf(),
    );
    let result = super::eval_expr(state, &format!("${{load(./{})}}", filename)).await?;
    let raw: T = serde_json::from_value(result.to_json())
        .map_err(|err| anyhow!("While loading type {}: {err}", std::any::type_name::<T>()))?;

    raw.load(state).await
}

#[async_trait::async_trait]
impl<T: DynValue> DynValue for std::collections::HashMap<String, T>
where
    T: Send,
    T::Target: Send,
{
    type Target = std::collections::HashMap<String, T::Target>;

    async fn load(self, state: &mut State) -> Result<Self::Target> {
        let mut res = std::collections::HashMap::new();

        let mut prev_id: Option<Value> = state
            .get_global()
            .map(|v| match v {
                Value::Dict(dict) => dict.get("_id").cloned(),
                _ => None,
            })
            .flatten();

        for (key, value) in self.into_iter() {
            // XXX: Get rid of this somehow. Now bind to legacy
            state.mutate_global(|value| {
                Ok(match value {
                    Value::Null => Value::Dict(HashMap::from_iter([(
                        "_id".to_string(),
                        key.clone().into(),
                    )])),
                    Value::Dict(mut dict) => {
                        dict.insert("_id".to_string(), key.clone().into());
                        Value::Dict(dict)
                    }
                    _ => value,
                })
            })?;
            res.insert(
                key.clone(),
                value
                    .load(state)
                    .await
                    .map_err(|err| anyhow!("While loading object field \"{key}\", {err}"))?,
            );
        }

        if let Some(prev_id) = prev_id.take() {
            state.mutate_global(move |value| {
                Ok(match value {
                    Value::Dict(mut dict) => {
                        dict.insert("_id".to_string(), prev_id);
                        Value::Dict(dict)
                    }
                    _ => value,
                })
            })?;
        }

        Ok(res)
    }
}

#[async_trait::async_trait]
impl<T: DynValue> DynValue for Vec<T>
where
    T: Send,
    T::Target: Send,
{
    type Target = Vec<T::Target>;

    async fn load(self, state: &mut State) -> Result<Self::Target> {
        let mut res = Vec::new();
        for (i, value) in self.into_iter().enumerate() {
            res.push(
                value
                    .load(state)
                    .await
                    .map_err(|err| anyhow!("While loading list index {i}: {err}"))?,
            );
        }
        Ok(res)
    }
}

#[async_trait::async_trait]
impl<T: DynValue> DynValue for Option<T>
where
    T: Send,
{
    type Target = Option<T::Target>;

    async fn load(self, state: &mut State) -> Result<Self::Target> {
        if let Some(value) = self {
            Ok(Some(value.load(state).await.map_err(|err| {
                anyhow!("While loading optional value: {err}")
            })?))
        } else {
            Ok(None)
        }
    }
}

#[async_trait::async_trait]
impl DynValue for bool {
    type Target = bool;

    async fn load(self, _state: &mut State) -> Result<Self::Target> {
        Ok(self)
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, Clone)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    Many(Vec<T>),
    One(T),
}

#[async_trait::async_trait]
impl<T: DynValue> DynValue for OneOrMany<T>
where
    T: Send,
    T::Target: Send,
{
    type Target = Vec<T::Target>;

    async fn load(self, state: &mut State) -> Result<Self::Target> {
        match self {
            OneOrMany::One(v) => Ok(vec![v.load(state).await?]),
            OneOrMany::Many(vs) => {
                let mut res = Vec::new();
                for v in vs.into_iter() {
                    res.push(v.load(state).await?);
                }
                Ok(res)
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(transparent)]
pub struct DynAny {
    value: serde_json::Value,
}

#[async_trait::async_trait]
impl DynValue for DynAny {
    type Target = Value;

    async fn load(self, state: &mut State) -> Result<Self::Target> {
        let value = Value::from_json(self.value)?;
        deep_eval(value, state).await
    }
}

fn deep_eval<'a, 'b, 'c>(value: Value, state: &'b mut State<'a>) -> BoxFuture<'c, Result<Value>>
where
    'a: 'b,
    'b: 'c,
{
    async move {
        match value {
            Value::String(s) => {
                let mut scope = state.scope();
                if let Ok(expr) = crate::parse_expr(&s) {
                    expr.eval(&mut scope).await
                } else if let Ok(fmt_string) = crate::parse_string(&s) {
                    fmt_string.eval(&mut scope).await
                } else {
                    Ok(Value::String(s))
                }
            }
            Value::Array(array) => {
                let mut res = Vec::new();

                for v in array.into_iter() {
                    match deep_eval(v, state).await {
                        Ok(v) => {
                            res.push(v);
                        }
                        err => {
                            return err;
                        }
                    }
                }

                Ok(Value::Array(res))
            }
            Value::Dict(dict) => {
                let mut res = HashMap::new();

                for (k, v) in dict.into_iter() {
                    match deep_eval(v, state).await {
                        Ok(v) => {
                            res.insert(k, v);
                        }
                        err => {
                            return err;
                        }
                    }
                }

                Ok(Value::Dict(res))
            }
            v @ (Value::Boolean(_) | Value::Null | Value::Integer(_)) => Ok(v),
        }
    }
    .boxed()
}
