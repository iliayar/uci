use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use anyhow::anyhow;

pub fn substitute_vars_dict(
    state: &super::State,
    dict: HashMap<String, String>,
) -> Result<HashMap<String, String>, super::LoadConfigError> {
    let vars: common::vars::Vars = state.into();
    let result: Result<_, super::LoadConfigError> = dict
        .into_iter()
        .map(|(k, v)| Ok((k, vars.eval(&v)?)))
        .collect();

    Ok(result?)
}

pub fn substitute_vars_list(
    state: &super::State,
    list: Vec<String>,
) -> Result<Vec<String>, super::LoadConfigError> {
    let vars: common::vars::Vars = state.into();
    let result: Result<_, super::LoadConfigError> =
        list.into_iter().map(|v| Ok(vars.eval(&v)?)).collect();

    Ok(result?)
}

pub fn substitute_vars(
    state: &super::State,
    s: String,
) -> Result<String, super::LoadConfigError> {
    let vars: common::vars::Vars = state.into();
    Ok(vars.eval(&s)?)
}

pub fn get_networks_names(
    state: &super::State,
    networks: Vec<String>,
) -> Result<Vec<String>, super::LoadConfigError> {
    let services: &super::Services = state.get()?;
    let project_info: &super::ProjectInfo = state.get()?;
    networks
        .into_iter()
        .map(|network| services.get_network_name(&project_info.id, network))
        .collect()
}

pub fn get_volumes_names(
    state: &super::State,
    volumes: HashMap<String, String>,
) -> Result<HashMap<String, String>, super::LoadConfigError> {
    let services: &super::Services = state.get()?;
    let project_info: &super::ProjectInfo = state.get()?;
    let volumes: Result<HashMap<_, _>, super::LoadConfigError> =
        substitute_vars_dict(state, volumes)?
            .into_iter()
            .map(|(k, v)| Ok((services.get_volume_name(&project_info.id, v)?, k)))
            .collect();

    Ok(volumes?)
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Enabled {
    Bool(bool),
    String(String),
}

impl super::LoadRawSync for Enabled {
    type Output = bool;

    fn load_raw(self, state: &super::State) -> Result<Self::Output, super::LoadConfigError> {
        match self {
            Enabled::Bool(v) => Ok(v),
            Enabled::String(s) => {
                let vars: common::vars::Vars = state.into();
                let s = vars.eval(&s)?;
                Ok(s.parse()
                    .map_err(|err| anyhow!("Failed to parse enable field: {}", err))?)
            }
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

    fn load_raw(self, state: &super::State) -> Result<Self::Output, super::LoadConfigError> {
        match self {
            AsString::Bool(v) => Ok(v.to_string()),
            AsString::String(s) => {
                let vars: common::vars::Vars = state.into();
                let s = vars.eval(&s)?;
                Ok(s)
            }
            AsString::Int(n) => Ok(n.to_string()),
        }
    }
}
