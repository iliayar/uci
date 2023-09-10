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

pub mod raw {
    use dynconf::*;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    use anyhow::Result;

    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct Bind {
        enabled: Option<util::Dyn<bool>>,
        zones: HashMap<String, Zone>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct Zone {
        ip: Option<String>,
        nameservers: Option<HashMap<String, String>>,
        cnames: Option<Vec<String>>,
        extra: Option<String>,
    }

    #[async_trait::async_trait]
    impl util::DynValue for Bind {
        type Target = Option<super::Bind>;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            if !self.enabled.load(state).await?.unwrap_or(true) {
                return Ok(None);
            }

            Ok(Some(super::Bind {
                zones: self.zones.load(state).await?,
            }))
        }
    }

    #[async_trait::async_trait]
    impl util::DynValue for Zone {
        type Target = super::Zone;

        async fn load(self, state: &mut State) -> Result<Self::Target> {
            Ok(super::Zone {
                ip: self.ip,
                nameservers: self.nameservers.unwrap_or_default(),
                cnames: self.cnames.unwrap_or_default(),
                extra: self.extra,
            })
        }
    }
}
