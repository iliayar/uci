
use std::path::PathBuf;

use tokio::io::AsyncWriteExt;

use crate::lib::config;

pub struct GenCaddy {
    pub config: Option<String>,
}

impl GenCaddy {
    pub fn is_empty(&self) -> bool {
	self.config.is_none()
    }

    pub async fn gen(self, path: PathBuf) -> Result<(), super::CodegenError> {
        if let Some(config) = self.config.as_ref() {
            let mut caddyfile = tokio::fs::File::create(path.join("Caddyfile")).await?;
            caddyfile.write_all(config.as_bytes()).await?;
        }

        Ok(())
    }
}
