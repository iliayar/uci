use serde::{Deserialize, Serialize};

use crate::{eval_expr, eval_string, prelude::*};

use std::{marker::PhantomData, path::PathBuf};

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

        Ok(serde_json::from_value::<T>(value.to_json())?
            .load(&mut state)
            .await?)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum Dyn<T> {
    Exact(T),
    Expression(String),
}

#[async_trait::async_trait]
impl<T: Send> DynValue for Dyn<T>
where
    T: for<'a> Deserialize<'a>,
    T: DynValue,
{
    type Target = T::Target;

    async fn load(self, state: &mut State) -> Result<Self::Target> {
        let mut state = state.scope();

        match self {
            Dyn::Exact(value) => value.load(&mut state).await,
            Dyn::Expression(expr) => {
                serde_json::from_value::<T>(eval_expr(&mut state, &expr).await?.to_json())?
                    .load(&mut state)
                    .await
            }
        }
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
#[serde(transparent)]
pub struct Lazy<T> {
    value: T,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct LoadedLazy<T> {
    pub(crate) value: T,
    pub(crate) current_dir: Option<PathBuf>,
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
        })
    }
}

#[async_trait::async_trait]
impl<T: DynValue> DynValue for LoadedLazy<T>
where
    T: Send,
{
    type Target = T::Target;

    async fn load(self, state: &mut State) -> Result<Self::Target> {
        let mut state = state.scope();
        if let Some(current_dir) = self.current_dir {
            state.set_current_dir(current_dir);
        }

        self.value.load(&mut state).await
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
    let raw: T = serde_json::from_value(result.to_json())?;

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
        for (key, value) in self.into_iter() {
            res.insert(key, value.load(state).await?);
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
        for value in self.into_iter() {
            res.push(value.load(state).await?);
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
            Ok(Some(value.load(state).await?))
        } else {
            Ok(None)
        }
    }
}
