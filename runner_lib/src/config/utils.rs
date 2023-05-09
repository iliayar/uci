use std::collections::HashMap;

use common::state::State;
use serde::{Deserialize, Serialize};

use anyhow::anyhow;
use serde_json::Map;

pub fn substitute_vars_dict(
    state: &State,
    dict: HashMap<String, String>,
) -> Result<HashMap<String, String>, anyhow::Error> {
    let vars: common::vars::Value = state_to_vars(state);
    let result: Result<_, anyhow::Error> = dict
        .into_iter()
        .map(|(k, v)| Ok((k, vars.eval(v)?)))
        .collect();

    result
}

pub fn substitute_vars_list(
    state: &State,
    list: Vec<String>,
) -> Result<Vec<String>, anyhow::Error> {
    let vars: common::vars::Value = state_to_vars(state);
    let result: Result<_, anyhow::Error> = list.into_iter().map(|v| Ok(vars.eval(v)?)).collect();

    result
}

pub fn substitute_vars<S: AsRef<str>>(state: &State, s: S) -> Result<String, anyhow::Error> {
    let vars: common::vars::Value = state_to_vars(state);
    Ok(vars.eval(s)?)
}

pub fn substitute_vars_json(
    state: &State,
    value: serde_json::Value,
) -> Result<serde_json::Value, anyhow::Error> {
    match value {
        serde_json::Value::Null => Ok(serde_json::Value::Null),
        serde_json::Value::Bool(v) => Ok(serde_json::Value::Bool(v)),
        serde_json::Value::Number(v) => Ok(serde_json::Value::Number(v)),
        serde_json::Value::String(s) => Ok(serde_json::Value::String(substitute_vars(state, s)?)),
        serde_json::Value::Array(arr) => {
            let arr: Result<_, anyhow::Error> = arr
                .into_iter()
                .map(|v| substitute_vars_json(state, v))
                .collect();
            Ok(serde_json::Value::Array(arr?))
        }
        serde_json::Value::Object(obj) => {
            let obj: Result<Map<String, serde_json::Value>, anyhow::Error> = obj
                .into_iter()
                .map(|(k, v)| Ok((k, substitute_vars_json(state, v)?)))
                .collect();
            Ok(serde_json::Value::Object(obj?))
        }
    }
}

pub fn get_networks_names(
    state: &State,
    networks: Vec<String>,
) -> Result<Vec<String>, anyhow::Error> {
    let services: &super::Services = state.get()?;
    let project_info: &super::ProjectInfo = state.get()?;
    networks
        .into_iter()
        .map(|network| services.get_network_name(&project_info.id, network))
        .collect()
}

pub fn get_volumes_names(
    state: &State,
    volumes: HashMap<String, String>,
) -> Result<HashMap<String, String>, anyhow::Error> {
    let services: &super::Services = state.get()?;
    let project_info: &super::ProjectInfo = state.get()?;
    let volumes: Result<HashMap<_, _>, anyhow::Error> = substitute_vars_dict(state, volumes)?
        .into_iter()
        .map(|(k, v)| Ok((services.get_volume_name(&project_info.id, v)?, k)))
        .collect();

    volumes
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Enabled {
    Bool(bool),
    String(String),
}

impl super::LoadRawSync for Enabled {
    type Output = bool;

    fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
        match self {
            Enabled::Bool(v) => Ok(v),
            Enabled::String(s) => Ok(substitute_vars(state, s)?
                .parse()
                .map_err(|err| anyhow!("Failed to parse enable field: {}", err))?),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum AsString {
    Bool(bool),
    String(String),
    Int(i64),
}

impl super::LoadRawSync for AsString {
    type Output = String;

    fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
        match self {
            AsString::Bool(v) => Ok(v.to_string()),
            AsString::String(s) => Ok(substitute_vars(state, s)?),
            AsString::Int(n) => Ok(n.to_string()),
        }
    }
}

fn state_to_vars(state: &State) -> common::vars::Value {
    use common::vars::*;
    let mut vars = Value::default();

    if let Ok(project_info) = state.get::<super::ProjectInfo>() {
        vars.assign("project", project_info.into()).ok();
    }

    if let Ok(config) = state.get::<super::ServiceConfig>() {
        vars.assign("config", config.into()).ok();
    }

    if let Ok(static_projects) = state.get::<super::StaticProjects>() {
        vars.assign("static_projects", static_projects.into()).ok();
    }

    if let Ok(project_params) = state.get_named::<HashMap<String, String>, _>("project_params") {
        for (key, value) in project_params.iter() {
            vars.assign(format!("params.{}", key), value.into()).ok();
        }
    }

    if let Ok(action_params) = state.get_named::<HashMap<String, String>, _>("action_params") {
        for (key, value) in action_params.iter() {
            vars.assign(format!("params.{}", key), value.into()).ok();
        }
    }

    if let Ok(pipeline_id) = state.get_named::<String, _>("pipeline_id") {
        vars.assign("pipeline.id", pipeline_id.into()).ok();
    }

    vars
}
