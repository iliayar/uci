use std::path::PathBuf;

use anyhow::anyhow;
use common::state::State;

use crate::config;

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
    let home_path = path
        .as_ref()
        .strip_prefix("~/")
        .ok_or_else(|| anyhow::anyhow!("No home prefix ~ in path '{}'", path.as_ref()))?;
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
    state: &State,
    path: S,
    dirpath: PathBuf,
) -> Result<PathBuf, anyhow::Error> {
    let path = config::utils::substitute_vars(state, path)?;

    if dirpath.is_dir() {
        Ok(abs_or_rel_to_dir(path, dirpath))
    } else {
        Ok(abs_or_rel_to_file(path, dirpath))
    }
}

pub fn eval_abs_path<S: AsRef<str>>(state: &State, path: S) -> Result<PathBuf, anyhow::Error> {
    let res_path = config::utils::substitute_vars(state, path.as_ref())?;
    let res_path = try_expand_home(res_path);

    if res_path.is_absolute() {
        Ok(res_path)
    } else {
        Err(anyhow!(
            "Expect path {} to be absolute, got {}",
            path.as_ref(),
            res_path.display()
        ))
    }
}
