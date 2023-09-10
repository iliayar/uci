// FIXME: This file is very personal, maybe make it more generic
use std::collections::HashMap;

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

pub mod raw {
    use dynconf::*;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    use anyhow::Result;

    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct Caddy {
        enabled: Option<util::Dyn<bool>>,
        hostnames: HashMap<String, util::DynString>,
    }

    #[async_trait::async_trait]
    impl util::DynValue for Caddy {
        type Target = Option<super::Caddy>;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            if !self.enabled.load(state).await?.unwrap_or(true) {
                return Ok(None);
            }

            let mut hostnames: HashMap<String, String> = HashMap::new();
            for (hostname, config) in self.hostnames.into_iter() {
                hostnames.insert(hostname, config.load(state).await?);
            }
            Ok(Some(super::Caddy { hostnames }))
        }
    }
}
