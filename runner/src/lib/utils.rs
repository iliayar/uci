use std::{error::Error, path::PathBuf};

use thiserror;

pub fn expand_home<S: AsRef<str>>(path: S) -> PathBuf {
    expand_home_impl(path).unwrap()
}

pub fn try_expand_home<S: AsRef<str>>(path: S) -> PathBuf {
    match expand_home_impl(path.as_ref()) {
        Err(_) => path.as_ref().into(),
        Ok(path) => path,
    }
}

pub fn expand_home_impl<S: AsRef<str>>(path: S) -> Result<PathBuf, anyhow::Error> {
    let home = std::env::var("HOME").expect("Should have HOME to be set");
    let home_path = path.as_ref().strip_prefix("~/").ok_or(anyhow::anyhow!(
        "No home prefix ~ in path '{}'",
        path.as_ref()
    ))?;
    Ok(PathBuf::from(format!("{}/{}", home, home_path)))
}
