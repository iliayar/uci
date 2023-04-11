use std::{collections::HashMap, path::PathBuf};

use tokio::io::AsyncWriteExt;

use crate::lib::config;

pub struct GenCaddy {
    pub hostnames: HashMap<String, String>,
}

impl GenCaddy {
    pub fn is_empty(&self) -> bool {
        self.hostnames.is_empty()
    }

    pub async fn gen(self, path: PathBuf) -> Result<(), super::CodegenError> {
        let mut caddyfile = tokio::fs::File::create(path.join("Caddyfile")).await?;
        for (hostname, config) in self.hostnames.into_iter() {
            caddyfile
                .write_all(
                    format!(
                        "{} {{
{}
}}",
                        hostname, config
                    )
                    .as_bytes(),
                )
                .await?;
        }

        Ok(())
    }
}
