use std::{collections::HashMap, path::PathBuf};

use anyhow::anyhow;

pub fn substitute_path_vars(
    context: &super::LoadContext,
    unprepared_links: HashMap<String, String>,
) -> Result<HashMap<String, String>, super::LoadConfigError> {
    let vars = context.get_vars();
    let result: Result<_, super::LoadConfigError> = unprepared_links
        .into_iter()
        .map(|(link, path)| Ok((link, vars.eval(&path)?)))
        .collect();

    Ok(result?)
}

fn substitute_path(substitutions: &HashMap<String, PathBuf>, path: String) -> String {
    for (var, subst) in substitutions {
        if let Some(rel_path) = path.strip_prefix(&format!("${}/", var)) {
            return subst.join(rel_path).to_string_lossy().to_string();
        }
    }

    path
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
        substitute_path_vars(context, volumes)?
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
