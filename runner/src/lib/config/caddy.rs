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
    config: Option<String>,
}

impl CaddyBuilder {
    pub fn add(&mut self, other: &Caddy) -> Result<(), LoadConfigError> {
        if let Some(cur_config) = self.config.as_mut() {
            cur_config.push_str(&other.config);
        } else {
            self.config = Some(other.config.clone());
        }

        Ok(())
    }

    pub fn build(self) -> super::codegen::caddy::GenCaddy {
        super::codegen::caddy::GenCaddy {
            config: self.config,
        }
    }
}

#[derive(Debug)]
pub struct Caddy {
    config: String,
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
        config: String,
    }

    impl config::LoadRawSync for Caddy {
        type Output = super::Caddy;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            let vars: common::vars::Vars = context.into();
            Ok(super::Caddy {
                config: vars.eval(&self.config)?,
            })
        }
    }

    impl config::AutoLoadRaw for Config {}

    pub async fn load<'a>(
        context: &config::LoadContext<'a>,
    ) -> Result<Option<super::Caddy>, super::LoadConfigError> {
        let path = context.project_config()?.clone();

        if path.exists() {
            if let Some(caddy) = config::load_sync::<Config>(path, context).await?.caddy {
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
