// FIXME: This file is very personal, maybe make it more generic
use std::collections::{HashMap, HashSet};

use anyhow::anyhow;
use common::state::State;

#[derive(Default)]
pub struct BindBuilder {
    zones: HashMap<String, ZoneBuilder>,
}

#[derive(Default)]
pub struct ZoneBuilder {
    ip: Option<String>,
    nameservers: HashMap<String, String>,
    cnames: HashSet<String>,
    extra: Option<String>,
}

impl ZoneBuilder {
    fn add(&mut self, zone: &Zone) -> Result<(), anyhow::Error> {
        if let Some(ip) = self.ip.as_ref() {
            if let Some(zone_ip) = zone.ip.as_ref() {
                if ip != zone_ip {
                    return Err(anyhow!("Zone ip do not match {} != {}", ip, zone_ip));
                }
            }
        } else {
            if let Some(zone_ip) = zone.ip.as_ref() {
                self.ip = Some(zone_ip.clone());
            }
        }

        for (ns, ip) in zone.nameservers.iter() {
            if let Some(oip) = self.nameservers.get(ns) {
                if ip != oip {
                    return Err(anyhow!("Nameserver ip do not match {} != {}", ip, oip));
                }
            }
            self.nameservers.insert(ns.clone(), ip.clone());
        }

        for name in zone.cnames.iter() {
            if self.cnames.contains(name) {
                return Err(anyhow!("cname {} already exists", name));
            }
            self.cnames.insert(name.clone());
        }

        if let Some(oextra) = zone.extra.as_ref() {
            if let Some(extra) = self.extra.as_mut() {
                extra.push_str(oextra);
            } else {
                self.extra = Some(oextra.to_string());
            }
        }

        Ok(())
    }

    pub fn build(self) -> super::codegen::bind::GenZone {
        super::codegen::bind::GenZone {
            ip: self.ip,
            nameservers: self.nameservers,
            cnames: self.cnames,
	    extra: self.extra,
        }
    }
}

impl BindBuilder {
    pub fn new(zones: HashMap<String, ZoneBuilder>) -> Self {
        Self { zones }
    }

    pub fn add(&mut self, bind: &Bind) -> Result<(), anyhow::Error> {
        for (zone, config) in bind.zones.iter() {
            if let Some(builder) = self.zones.get_mut(zone) {
                builder.add(config)?;
            } else {
                let mut builder = ZoneBuilder::default();
                builder.add(config)?;
                self.zones.insert(zone.clone(), builder);
            }
        }
        Ok(())
    }

    pub fn build(self) -> super::codegen::bind::GenBind {
        super::codegen::bind::GenBind {
            zones: self
                .zones
                .into_iter()
                .map(|(k, v)| (k, v.build()))
                .collect(),
        }
    }
}

#[derive(Debug)]
pub struct Bind {
    zones: HashMap<String, Zone>,
}

#[derive(Debug)]
pub struct Zone {
    ip: Option<String>,
    nameservers: HashMap<String, String>,
    cnames: Vec<String>,
    extra: Option<String>,
}

impl Bind {
    pub async fn load<'a>(state: &State<'a>) -> Result<Option<Bind>, anyhow::Error> {
        raw::load(state).await
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use common::state::State;
    use serde::{Deserialize, Serialize};

    use crate::config::{self, LoadRawSync};

    #[derive(Serialize, Deserialize)]
    pub struct Config {
        bind9: Option<Bind>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Bind {
        enabled: Option<bool>,
        zones: HashMap<String, Zone>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Zone {
        ip: Option<String>,
        nameservers: Option<HashMap<String, String>>,
        cnames: Option<Vec<String>>,
        extra: Option<String>,
    }

    impl config::LoadRawSync for Bind {
        type Output = super::Bind;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            Ok(super::Bind {
                zones: self.zones.load_raw(state)?,
            })
        }
    }

    impl config::LoadRawSync for Zone {
        type Output = super::Zone;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            Ok(super::Zone {
                ip: self.ip,
                nameservers: self.nameservers.unwrap_or_default(),
                cnames: self.cnames.unwrap_or_default(),
                extra: self.extra,
            })
        }
    }

    impl config::LoadRawSync for Config {
        type Output = Config;

        fn load_raw(self, state: &State) -> Result<Self::Output, anyhow::Error> {
            Ok(self)
        }
    }

    pub async fn load<'a>(state: &State<'a>) -> Result<Option<super::Bind>, anyhow::Error> {
        let path: PathBuf = state.get_named("project_config").cloned()?;

        if path.exists() {
            let config: Result<Config, anyhow::Error> =
                config::load_sync::<Config>(path.clone(), state)
                    .await
                    .map_err(|err| anyhow::anyhow!("Failed to load bind from {:?}: {}", path, err));
            if let Some(bind9) = config?.bind9 {
                if bind9.enabled.unwrap_or(true) {
                    Ok(Some(bind9.load_raw(state)?))
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
