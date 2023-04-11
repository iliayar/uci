// FIXME: This file is very personal, maybe make it more generic
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use tokio::io::AsyncWriteExt;

use super::LoadConfigError;

use anyhow::anyhow;

#[derive(Default)]
pub struct CaddyBuilder {
    hostnames: HashMap<String, String>,
}

impl CaddyBuilder {
    pub fn add(&mut self, other: &Caddy) -> Result<(), LoadConfigError> {
        for (hostname, config) in other.hostnames.iter() {
            if let Some(current_config) = self.hostnames.get_mut(hostname) {
                current_config.push_str(config);
            } else {
                self.hostnames.insert(hostname.clone(), config.clone());
            }
        }

        Ok(())
    }

    pub fn build(self) -> super::codegen::caddy::GenCaddy {
        super::codegen::caddy::GenCaddy {
            hostnames: self.hostnames,
        }
    }
}

#[derive(Debug)]
pub struct Caddy {
    hostnames: HashMap<String, String>,
}

impl Caddy {
    pub async fn load<'a>(
        context: &super::LoadContext<'a>,
    ) -> Result<Option<Caddy>, LoadConfigError> {
        raw::load(context).await
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use anyhow::anyhow;

    use crate::lib::{
        config::{self, LoadContext, LoadRawSync},
        utils,
    };

    #[derive(Serialize, Deserialize)]
    pub struct Config {
        caddy: Option<Caddy>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Caddy {
        enabled: Option<config::utils::Enabled>,
        hostnames: HashMap<String, String>,
    }

    impl config::LoadRawSync for Caddy {
        type Output = super::Caddy;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            let vars: common::vars::Vars = context.into();
            let hostnames: Result<HashMap<_, _>, config::LoadConfigError> = self
                .hostnames
                .into_iter()
                .map(|(hostname, config)| Ok((hostname, vars.eval(&config)?)))
                .collect();
            Ok(super::Caddy {
                hostnames: hostnames?,
            })
        }
    }

    impl config::AutoLoadRaw for Config {}

    pub async fn load<'a>(
        context: &config::LoadContext<'a>,
    ) -> Result<Option<super::Caddy>, super::LoadConfigError> {
        let path: PathBuf = context.get_named("project_config").cloned()?;
        if path.exists() {
            let config: Result<Config, super::LoadConfigError> =
                config::load_sync::<Config>(path.clone(), context)
                    .await
                    .map_err(|err| anyhow!("Failed to load caddy from {:?}: {}", path, err).into());
            if let Some(caddy) = config?.caddy {
                if caddy.enabled.clone().load_raw(context)?.unwrap_or(true) {
                    Ok(Some(caddy.load_raw(context)?))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}
