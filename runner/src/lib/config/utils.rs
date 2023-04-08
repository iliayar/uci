use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use anyhow::anyhow;

pub fn substitute_vars_dict(
    context: &super::LoadContext,
    dict: HashMap<String, String>,
) -> Result<HashMap<String, String>, super::LoadConfigError> {
    let vars: common::vars::Vars = context.into();
    let result: Result<_, super::LoadConfigError> = dict
        .into_iter()
        .map(|(k, v)| Ok((k, vars.eval(&v)?)))
        .collect();

    Ok(result?)
}

pub fn substitute_vars_list(
    context: &super::LoadContext,
    list: Vec<String>,
) -> Result<Vec<String>, super::LoadConfigError> {
    let vars: common::vars::Vars = context.into();
    let result: Result<_, super::LoadConfigError> =
        list.into_iter().map(|v| Ok(vars.eval(&v)?)).collect();

    Ok(result?)
}

pub fn substitute_vars(
    context: &super::LoadContext,
    s: String,
) -> Result<String, super::LoadConfigError> {
    let vars: common::vars::Vars = context.into();
    Ok(vars.eval(&s)?)
}

pub fn get_networks_names(
    context: &super::LoadContext,
    networks: Vec<String>,
) -> Result<Vec<String>, super::LoadConfigError> {
    networks
        .into_iter()
        .map(|network| get_network_name(context, network))
        .collect()
}

pub fn get_volumes_names(
    context: &super::LoadContext,
    volumes: HashMap<String, String>,
) -> Result<HashMap<String, String>, super::LoadConfigError> {
    let volumes: Result<HashMap<_, _>, super::LoadConfigError> =
        substitute_vars_dict(context, volumes)?
            .into_iter()
            .map(|(k, v)| Ok((get_volume_name(context, v)?, k)))
            .collect();

    Ok(volumes?)
}

pub fn get_network_name(
    context: &super::LoadContext,
    network: String,
) -> Result<String, super::LoadConfigError> {
    let global = context
        .networks()?
        .get(&network)
        .ok_or(anyhow!("No such network {}", network))?
        .global;
    Ok(get_resource_name(context.project_id()?, network, global))
}

pub fn get_volume_name(
    context: &super::LoadContext,
    volume: String,
) -> Result<String, super::LoadConfigError> {
    if let Some(v) = context.volumes()?.get(&volume) {
        Ok(get_resource_name(context.project_id()?, volume, v.global))
    } else {
        Ok(volume)
    }
}

fn get_resource_name(project_id: &str, name: String, global: bool) -> String {
    if global {
        name
    } else {
        format!("{}_{}", project_id, name)
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Enabled {
    Bool(bool),
    String(String),
}

impl super::LoadRawSync for Enabled {
    type Output = bool;

    fn load_raw(
        self,
        context: &super::LoadContext,
    ) -> Result<Self::Output, super::LoadConfigError> {
        match self {
            Enabled::Bool(v) => Ok(v),
            Enabled::String(s) => {
                let vars: common::vars::Vars = context.into();
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

    fn load_raw(
        self,
        context: &super::LoadContext,
    ) -> Result<Self::Output, super::LoadConfigError> {
        match self {
            AsString::Bool(v) => Ok(v.to_string()),
            AsString::String(s) => {
                let vars: common::vars::Vars = context.into();
                let s = vars.eval(&s)?;
                Ok(s)
            }
	    AsString::Int(n) => Ok(n.to_string()),
        }
    }
}
