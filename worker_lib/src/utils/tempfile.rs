use std::{
    convert::Infallible, env::temp_dir, fs::Permissions, os::unix::prelude::PermissionsExt,
    path::PathBuf,
};

use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use log::*;

pub struct TempFile {
    pub path: PathBuf,
    should_delete: bool,
    // FIXME: Carry out to different struct?
    is_dir: bool,
}

impl TempFile {
    pub async fn empty() -> TempFile {
        TempFile {
            path: get_temp_filename().await,
            should_delete: true,
            is_dir: false,
        }
    }

    pub async fn copy(path: PathBuf) -> Result<TempFile, tokio::io::Error> {
        let file_path = get_temp_filename().await;
        tokio::fs::copy(&path, &file_path).await?;

        Ok(TempFile {
            path: file_path,
            should_delete: true,
            is_dir: false,
        })
    }

    pub async fn dir() -> Result<TempFile, tokio::io::Error> {
        let dir_path = get_temp_filename().await;
        tokio::fs::create_dir(&dir_path).await?;

        Ok(TempFile {
            path: dir_path,
            should_delete: true,
            is_dir: true,
        })
    }

    pub async fn dummy(path: PathBuf) -> TempFile {
        TempFile {
            path,
            should_delete: false,
            is_dir: false,
        }
    }

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
        let file_path = get_temp_filename().await;
        let mut file = tokio::fs::File::create(file_path.clone()).await?;

        if let Some(perms) = perms {
            file.set_permissions(perms).await?;
        }

        file.write_all(text.as_bytes()).await?;

        Ok(TempFile {
            path: file_path,
            should_delete: true,
            is_dir: false,
        })
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if !self.should_delete {
            return;
        }

        let result = if self.is_dir {
            std::fs::remove_dir_all(self.path.clone())
        } else {
            std::fs::remove_file(self.path.clone())
        };

        if let Err(err) = result {
            error!(
                "Failed to remove temp file {:?}: {}",
                self.path.as_path(),
                err
            );
        }
    }
}

async fn get_temp_filename() -> PathBuf {
    temp_dir().join(format!("microci-tmp-{}", Uuid::new_v4()))
}
