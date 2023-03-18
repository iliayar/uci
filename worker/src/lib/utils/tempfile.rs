use std::{env::temp_dir, fs::Permissions, os::unix::prelude::PermissionsExt, path::PathBuf};

use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use log::*;

pub struct TempFile {
    pub path: PathBuf,
}

impl TempFile {
    pub async fn new(text: &str) -> Result<TempFile, tokio::io::Error> {
        TempFile::new_permissions(text, None).await
    }

    pub async fn new_executable(text: &str) -> Result<TempFile, tokio::io::Error> {
        TempFile::new_permissions(text, Some(Permissions::from_mode(0o700))).await
    }

    pub async fn new_permissions(
        text: &str,
        perms: Option<Permissions>,
    ) -> Result<TempFile, tokio::io::Error> {
        let filename = format!("microci-tmp-{}", Uuid::new_v4());
        let file_path = temp_dir().join(filename);
        let mut file = tokio::fs::File::create(file_path.clone()).await?;

        if let Some(perms) = perms {
            file.set_permissions(perms).await?;
        }

        file.write_all(text.as_bytes()).await?;

        Ok(TempFile { path: file_path })
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let result = std::fs::remove_file(self.path.clone());

        if let Err(err) = result {
            error!(
                "Failed to remove temp file {:?}: {}",
                self.path.as_path(),
                err
            );
        }
    }
}

pub async fn get_temp_filename() -> PathBuf {
    temp_dir().join(format!("microci-tmp-{}", Uuid::new_v4()))
}
