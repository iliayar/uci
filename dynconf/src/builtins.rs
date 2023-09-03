use std::collections::HashMap;

use crate::prelude::*;

pub fn register_builtins(state: &mut State) -> Result<()> {
    state.register_function("list", List::new())?;
    state.register_function("dict", Dict::new())?;

    #[cfg(all(feature = "yaml", feature = "io"))]
    {
        state.register_function("load", load_yaml::LoadYaml::new())?;
    }

    Ok(())
}

struct List {}
#[async_trait::async_trait]
impl Function for List {
    async fn call<'a>(&self, state: &mut State, args: Vec<LExpression<'a>>) -> Result<Value> {
        let mut list = Vec::new();
        for arg in args.into_iter() {
            list.push(arg.eval(state).await?);
        }
        Ok(Value::Array(list))
    }
}

impl List {
    pub fn new() -> Self {
        Self {}
    }
}

struct Dict {}
#[async_trait::async_trait]
impl Function for Dict {
    async fn call<'a>(&self, state: &mut State, mut args: Vec<LExpression<'a>>) -> Result<Value> {
        if args.len() % 2 != 0 {
            return Err(anyhow!("Arguments count must be even"));
        }

        let mut res: HashMap<String, Value> = HashMap::default();
        while !args.is_empty() {
            let v = args.pop().unwrap();
            let k = args.pop().unwrap();

            let k = k.eval(state).await?.try_to_string()?;
            let v = v.eval(state).await?;

            res.insert(k, v);
        }

        Ok(Value::Dict(res))
    }
}

impl Dict {
    pub fn new() -> Self {
        Self {}
    }
}

#[cfg(all(feature = "yaml", feature = "io"))]
mod load_yaml {
    use std::path::PathBuf;

    use super::*;

    pub struct LoadYaml {}
    #[async_trait::async_trait]
    impl Function for LoadYaml {
        async fn call<'a>(
            &self,
            state: &mut State,
            mut args: Vec<LExpression<'a>>,
        ) -> Result<Value> {
            if args.len() != 1 {
                return Err(anyhow!("Expected 1 argument (path) for load yaml"));
            }

            let filename: &PathBuf = &args
                .pop()
                .unwrap()
                .eval(state)
                .await?
                .try_to_string()?
                .into();

            let new_dir = filename
                .parent()
                .ok_or_else(|| anyhow!("Cannot load file, because file has no parent dir"))?;

            state.set_current_dir(new_dir.into());

            let file_content = tokio::fs::read_to_string(filename).await?;
            let value = serde_yaml::from_str::<serde_yaml::Value>(&file_content)?;

            Value::from_yaml(value)
        }
    }

    impl LoadYaml {
        pub fn new() -> Self {
            Self {}
        }
    }
}
