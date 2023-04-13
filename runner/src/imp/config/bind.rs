// FIXME: This file is very personal, maybe make it more generic
use std::collections::{HashMap, HashSet};

use anyhow::anyhow;

#[derive(Default)]
pub struct BindBuilder {
    zones: HashMap<String, ZoneBuilder>,
}

#[derive(Default)]
pub struct ZoneBuilder {
    ip: Option<String>,
    nameservers: HashMap<String, String>,
    cnames: HashSet<String>,
}

impl ZoneBuilder {
    fn add(&mut self, zone: &Zone) -> Result<(), anyhow::Error> {
        if let Some(ip) = self.ip.as_ref() {
            if ip != &zone.ip {
                return Err(anyhow!("Zone ip do not match {} != {}", ip, zone.ip));
            }
        } else {
            self.ip = Some(zone.ip.clone());
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

        Ok(())
    }

    pub fn build(self) -> super::codegen::bind::GenZone {
        super::codegen::bind::GenZone {
            ip: self.ip,
            nameservers: self.nameservers,
            cnames: self.cnames,
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
    ip: String,
    nameservers: HashMap<String, String>,
    cnames: Vec<String>,
}

impl Bind {
    pub async fn load<'a>(context: &super::State<'a>) -> Result<Option<Bind>, anyhow::Error> {
        raw::load(context).await
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::imp::config::{self, LoadRawSync};

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
        ip: String,
        nameservers: Option<HashMap<String, String>>,
        cnames: Option<Vec<String>>,
    }

    impl config::LoadRawSync for Bind {
        type Output = super::Bind;

        fn load_raw(self, context: &config::State) -> Result<Self::Output, anyhow::Error> {
            Ok(super::Bind {
                zones: self.zones.load_raw(context)?,
            })
        }
    }

    impl config::LoadRawSync for Zone {
        type Output = super::Zone;

        fn load_raw(self, context: &config::State) -> Result<Self::Output, anyhow::Error> {
            Ok(super::Zone {
                ip: self.ip,
                nameservers: self.nameservers.unwrap_or_default(),
                cnames: self.cnames.unwrap_or_default(),
            })
        }
    }

    impl config::AutoLoadRaw for Config {}

    pub async fn load<'a>(
        context: &config::State<'a>,
    ) -> Result<Option<super::Bind>, anyhow::Error> {
        let path: PathBuf = context.get_named("project_config").cloned()?;

        if path.exists() {
            let config: Result<Config, anyhow::Error> =
                config::load_sync::<Config>(path.clone(), context)
                    .await
                    .map_err(|err| anyhow::anyhow!("Failed to load bind from {:?}: {}", path, err));
            if let Some(bind9) = config?.bind9 {
                if bind9.enabled.unwrap_or(true) {
                    Ok(Some(bind9.load_raw(context)?))
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
