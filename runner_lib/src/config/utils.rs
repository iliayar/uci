use anyhow::{anyhow, Result};
use common::state::State;
use dynconf::*;
use serde::{Deserialize, Serialize};

use crate::config;

pub struct Env(pub String);

#[derive(Deserialize, Serialize)]
pub struct DynObject {
    pub _id: Option<String>,

    pub config: Option<config::service_config::DynServiceConfig>,
    pub project: Option<config::projects::DynProjectInfo>,
    pub services: Option<config::services::DynServices>,
    pub params: Value,

    pub env: String,
}

pub fn wrap_dyn_f(f: impl Fn(DynObject) -> Result<DynObject>) -> impl Fn(Value) -> Result<Value> {
    Value::wrap_fun_t(f)
}

pub fn make_dyn_state(state: &State) -> Result<dynconf::State<'static>> {
    let dynobj = DynObject {
        _id: None,
        config: state
            .get::<config::service_config::ServiceConfig>()
            .map(Into::into)
            .ok(),
        project: state
            .get::<config::projects::ProjectInfo>()
            .map(Into::into)
            .ok(),
        services: state
            .get::<config::services::Services>()
            .map(Into::into)
            .ok(),
        params: Value::Null,
        env: state.get::<Env>()?.0.clone(),
    };

    let mut state = dynconf::State::initialize();
    state.set_global(Value::from_t(dynobj)?);
    Ok(state)
}

pub fn get_dyn_object(state: &dynconf::State) -> Result<DynObject> {
    Value::to_t(
        state
            .get_global()
            .ok_or_else(|| anyhow!("No global object in state"))?
            .clone(),
    )
}
