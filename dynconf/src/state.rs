use std::{collections::HashMap, path::PathBuf, sync::Arc};

use crate::prelude::*;

#[derive(Default)]
struct StateContent {
    functions: HashMap<String, Arc<dyn Function>>,
    global: Option<Value>,
    current_dir: Option<PathBuf>,
}

pub struct State<'a> {
    parent: Option<&'a State<'a>>,
    content: StateContent,
}

impl<'a> State<'a> {
    pub fn initialize() -> State<'a> {
        let mut state = State {
            parent: None,
            content: StateContent::default(),
        };

        crate::builtins::register_builtins(&mut state).expect("Builtins guarantee to not overlap");

        state
    }

    pub fn scope(&self) -> State {
        State {
            parent: Some(self),
            content: StateContent::default(),
        }
    }

    pub fn register_function<F: Function + 'static>(
        &mut self,
        name: impl AsRef<str>,
        function: F,
    ) -> Result<()> {
        self.content.register_function(name, function)
    }

    fn get_function(&self, name: impl AsRef<str>) -> Result<Arc<dyn Function>> {
        if let Some(function) = self.content.get_function(name.as_ref()) {
            Ok(function.clone())
        } else if let Some(parent) = self.parent {
            parent.get_function(name)
        } else {
            Err(anyhow!("No such function: {}", name.as_ref()))
        }
    }

    pub async fn call_function<'b>(
        &mut self,
        name: impl AsRef<str>,
        args: Vec<LExpression<'b>>,
    ) -> Result<Value> {
        let function = self.get_function(name)?;
        function.call(self, args).await
    }

    pub fn set_global(&mut self, value: Value) {
        self.content.set_global(value);
    }

    pub fn get_global(&self) -> Option<&Value> {
        if let Some(value) = self.content.get_global() {
            Some(value)
        } else if let Some(parent) = self.parent {
            parent.get_global()
        } else {
            None
        }
    }

    pub fn mutate_global(&mut self, f: impl FnOnce(Value) -> Result<Value>) -> Result<()> {
        let global = if let Some(value) = self.content.global.take() {
            value
        } else if let Some(parent) = self.parent {
            parent.get_global().cloned().unwrap_or(Value::Null)
        } else {
            Value::Null
        };

        self.content.global = Some(f(global)?);

        Ok(())
    }

    pub fn set_current_dir(&mut self, current_dir: PathBuf) {
        self.content.set_current_dir(current_dir);
    }

    pub fn get_current_dir(&self) -> Option<PathBuf> {
        if let Some(current_dir) = self.content.get_current_dir() {
            Some(current_dir)
        } else if let Some(parent) = self.parent {
            parent.get_current_dir()
        } else {
            None
        }
    }
}

impl StateContent {
    fn register_function<F: Function + 'static>(
        &mut self,
        name: impl AsRef<str>,
        function: F,
    ) -> Result<()> {
        if self.functions.contains_key(name.as_ref()) {
            return Err(anyhow!("Function {} already registered", name.as_ref()));
        }

        self.functions
            .insert(name.as_ref().to_string(), Arc::new(function));

        Ok(())
    }

    fn get_function(&self, name: impl AsRef<str>) -> Option<Arc<dyn Function>> {
        self.functions.get(name.as_ref()).cloned()
    }

    fn set_global(&mut self, value: Value) {
        self.global = Some(value);
    }

    fn get_global(&self) -> Option<&Value> {
        self.global.as_ref()
    }

    pub fn set_current_dir(&mut self, current_dir: PathBuf) {
        self.current_dir = Some(current_dir);
    }

    pub fn get_current_dir(&self) -> Option<PathBuf> {
        self.current_dir.clone()
    }
}
