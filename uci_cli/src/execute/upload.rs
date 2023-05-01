use std::path::PathBuf;

use log::*;
use termion::{color, style};

pub async fn execute_upload(
    config: &crate::config::Config,
    path: PathBuf,
) -> Result<(), super::ExecuteError> {
    debug!("Executing upload");

    let data = tokio::fs::read(&path).await.map_err(|err| {
        super::ExecuteError::Fatal(format!("Cannot uppload file {}: {}", path.display(), err))
    })?;
    let response = crate::runner::api::upload(config, data).await?;

    println!(
        "{}Uploaded artifact: {}{}",
        color::Fg(color::Green),
        response.artifact,
        style::Reset
    );

    Ok(())
}
