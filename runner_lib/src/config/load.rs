use std::{collections::HashMap, path::PathBuf};

use common::state::State;

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

pub trait AutoLoadRaw {}

impl<T: AutoLoadRaw> LoadRawSync for T {
    type Output = T;

    fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
        Ok(self)
    }
}

#[async_trait::async_trait]
impl<T: AutoLoadRaw + Send> LoadRaw for T {
    type Output = T;

    async fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
        Ok(self)
    }
}
