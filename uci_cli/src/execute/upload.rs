use std::path::PathBuf;

use log::*;

pub async fn execute_upload(
    config: &crate::config::Config,
    path: PathBuf,
) -> Result<(), super::ExecuteError> {
    debug!("Executing upload");

    super::utils::upload_archive(config, path).await?;

    Ok(())
}
