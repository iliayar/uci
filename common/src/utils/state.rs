use std::{collections::HashMap, sync::Arc};

use anyhow::anyhow;

#[derive(Default, Clone)]
pub struct State<'a> {
    // FIXME: I'm pretty sure it's possible to not having separate
    // maps for owned and borrowed and somehow store reference to self
    // using Pin somewhere
    owned_typed: HashMap<std::any::TypeId, Arc<dyn std::any::Any + Sync + Send>>,

    typed: HashMap<std::any::TypeId, &'a (dyn std::any::Any + Sync)>,
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
}
