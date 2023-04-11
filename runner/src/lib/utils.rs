use std::{error::Error, path::PathBuf};

use anyhow::anyhow;
use thiserror;

fn expand_home<S: AsRef<str>>(path: S) -> PathBuf {
    expand_home_impl(path).unwrap()
}

pub fn try_expand_home<S: AsRef<str>>(path: S) -> PathBuf {
    match expand_home_impl(path.as_ref()) {
        Err(_) => path.as_ref().into(),
        Ok(path) => path,
    }
}

fn expand_home_impl<S: AsRef<str>>(path: S) -> Result<PathBuf, anyhow::Error> {
    let home = std::env::var("HOME").expect("Should have HOME to be set");
    let home_path = path.as_ref().strip_prefix("~/").ok_or(anyhow::anyhow!(
        "No home prefix ~ in path '{}'",
        path.as_ref()
    ))?;
    Ok(PathBuf::from(format!("{}/{}", home, home_path)))
}

fn abs_or_rel_to_dir<S: AsRef<str>>(path: S, dirpath: PathBuf) -> PathBuf {
    assert!(dirpath.is_dir(), "Base path must be a directory");

    let path = try_expand_home(path);

    if path.is_absolute() {
        path
    } else {
        dirpath.join(path)
    }
}

fn abs_or_rel_to_file<S: AsRef<str>>(path: S, filepath: PathBuf) -> PathBuf {
    assert!(filepath.is_file(), "Base path must be a file");
    abs_or_rel_to_dir(path, filepath.parent().unwrap().to_path_buf())
}

pub fn eval_rel_path<S: AsRef<str>>(
    context: &super::config::LoadContext,
    path: S,
    dirpath: PathBuf,
) -> Result<PathBuf, super::config::LoadConfigError> {
    let vars: common::vars::Vars = context.into();
    let path = vars.eval(path.as_ref())?;

    if dirpath.is_dir() {
        Ok(abs_or_rel_to_dir(path, dirpath))
    } else {
        Ok(abs_or_rel_to_file(path, dirpath))
    }
}

pub fn eval_abs_path<S: AsRef<str>>(
    context: &super::config::LoadContext,
    path: S,
) -> Result<PathBuf, super::config::LoadConfigError> {
    let vars: common::vars::Vars = context.into();
    let res_path = vars.eval(path.as_ref())?;
    let res_path = try_expand_home(res_path);

    if res_path.is_absolute() {
        Ok(res_path)
    } else {
        Err(anyhow!("Epect path {} to be absolute", path.as_ref()).into())
    }
}
