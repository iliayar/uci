use std::{collections::HashMap, path::PathBuf};

use tokio::io::AsyncWriteExt;

pub struct GenCaddy {
    pub hostnames: HashMap<String, String>,
}

impl GenCaddy {
    pub fn is_empty(&self) -> bool {
        self.hostnames.is_empty()
    }

    pub async fn gen(self, path: PathBuf) -> Result<(), anyhow::Error> {
        let mut caddyfile = tokio::fs::File::create(path.join("Caddyfile")).await?;
        for (hostname, config) in self.hostnames.into_iter() {
            caddyfile
                .write_all(
                    format!(
                        r#"
{} {{
{}
}}
"#,
                        hostname, config
                    )
                    .as_bytes(),
                )
                .await?;
        }

        Ok(())
    }
}
