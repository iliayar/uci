// FIXME: This file is very personal, maybe make it more generic
use std::collections::HashMap;

use common::state::State;

#[derive(Default)]
pub struct CaddyBuilder {
    hostnames: HashMap<String, String>,
}

impl CaddyBuilder {
    pub fn add(&mut self, other: &Caddy) -> Result<(), anyhow::Error> {
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
    pub async fn load<'a>(state: &State<'a>) -> Result<Option<Caddy>, anyhow::Error> {
        raw::load(state).await
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use common::state::State;
    use serde::{Deserialize, Serialize};

    use anyhow::anyhow;

    use crate::config::{self, Expr, LoadRawSync};

    #[derive(Serialize, Deserialize)]
    pub struct Config {
        caddy: Option<Caddy>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Caddy {
        enabled: Option<Expr<bool>>,
        hostnames: HashMap<String, String>,
    }

    impl config::LoadRawSync for Caddy {
        type Output = super::Caddy;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            let hostnames: Result<HashMap<_, _>, anyhow::Error> = self
                .hostnames
                .into_iter()
                .map(|(hostname, config)| {
                    Ok((hostname, config::utils::substitute_vars(state, config)?))
                })
                .collect();
            Ok(super::Caddy {
                hostnames: hostnames?,
            })
        }
    }

    impl config::AutoLoadRaw for Config {}

    pub async fn load<'a>(state: &State<'a>) -> Result<Option<super::Caddy>, anyhow::Error> {
        let path: PathBuf = state.get_named("project_config").cloned()?;
        if path.exists() {
            let config: Result<Config, anyhow::Error> =
                config::load_sync::<Config>(path.clone(), state)
                    .await
                    .map_err(|err| anyhow!("Failed to load caddy from {:?}: {}", path, err));
            if let Some(caddy) = config?.caddy {
                if caddy.enabled.clone().load_raw(state)?.unwrap_or(true) {
                    Ok(Some(caddy.load_raw(state)?))
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
