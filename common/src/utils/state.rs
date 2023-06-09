use std::{collections::HashMap, sync::Arc};

use anyhow::anyhow;

#[derive(Default, Clone)]
pub struct State<'a> {
    // FIXME: I'm pretty sure it's possible to not having separate
    // maps for owned and borrowed and somehow store reference to self
    // using Pin somewhere
    owned_typed: HashMap<std::any::TypeId, Arc<dyn std::any::Any + Sync + Send>>,
    owned_named: HashMap<String, Arc<dyn std::any::Any + Sync + Send>>,

    typed: HashMap<std::any::TypeId, &'a (dyn std::any::Any + Sync)>,
    named: HashMap<String, &'a (dyn std::any::Any + Sync)>,
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
                // By this key there is exact type T
                None => unreachable!(),
            },
            None => self.get_owned(),
        }
    }

    fn get_owned<T: std::any::Any + Sync>(&self) -> Result<&T, anyhow::Error> {
        let type_id = std::any::TypeId::of::<T>();
        match self.owned_typed.get(&type_id) {
            Some(v) => match (v.as_ref() as &dyn std::any::Any).downcast_ref::<T>() {
                Some(v) => Ok(v),
                // By this key there is exact type T
                None => unreachable!(),
            },
            None => Err(anyhow!(
                "No type {} in load context",
                std::any::type_name::<T>()
            )),
        }
    }

    pub fn set_owned<T: std::any::Any + Sync + Send>(&mut self, value: T) {
        self.owned_typed
            .insert(std::any::TypeId::of::<T>(), Arc::new(value));
    }

    pub fn set_named<T: std::any::Any + Sync, S: AsRef<str>>(&mut self, key: S, value: &'a T) {
        self.named.insert(
            key.as_ref().to_string(),
            value as &(dyn std::any::Any + Sync),
        );
    }

    pub fn set_named_owned<T: std::any::Any + Sync + Send, S: AsRef<str>>(
        &mut self,
        key: S,
        value: T,
    ) {
        self.owned_named
            .insert(key.as_ref().to_string(), Arc::new(value));
    }

    pub fn get_named<T: std::any::Any, S: AsRef<str>>(&self, key: S) -> Result<&T, anyhow::Error> {
        match self.named.get(key.as_ref()) {
            Some(v) => match (*v as &dyn std::any::Any).downcast_ref::<T>() {
                Some(v) => Ok(v),
                None => Err(anyhow!(
                    "Value for {} has type different from {}",
                    key.as_ref(),
                    std::any::type_name::<T>()
                )),
            },
            None => self.get_named_owned(key),
        }
    }

    fn get_named_owned<T: std::any::Any, S: AsRef<str>>(
        &self,
        key: S,
    ) -> Result<&T, anyhow::Error> {
        match self.owned_named.get(key.as_ref()) {
            Some(v) => match (v.as_ref() as &dyn std::any::Any).downcast_ref::<T>() {
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
