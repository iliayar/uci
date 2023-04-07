// FIXME: This file is very personal, maybe make it more generic
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use tokio::io::AsyncWriteExt;

use super::LoadConfigError;

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
    fn add(&mut self, zone: &Zone) -> Result<(), LoadConfigError> {
        if let Some(ip) = self.ip.as_ref() {
            if ip != &zone.ip {
                return Err(anyhow!("Zone ip do not match {} != {}", ip, zone.ip).into());
            }
        } else {
            self.ip = Some(zone.ip.clone());
        }

        for (ns, ip) in zone.nameservers.iter() {
            if let Some(oip) = self.nameservers.get(ns) {
                if ip != oip {
                    return Err(anyhow!("Nameserver ip do not match {} != {}", ip, oip).into());
                }
            }
            self.nameservers.insert(ns.clone(), ip.clone());
        }

        for name in zone.cnames.iter() {
            if self.cnames.contains(name) {
                return Err(anyhow!("cname {} already exists", name).into());
            }
            self.cnames.insert(name.clone());
        }

        Ok(())
    }
}

impl BindBuilder {
    pub fn new(zones: HashMap<String, ZoneBuilder>) -> Self {
        Self { zones }
    }

    pub fn add(&mut self, bind: &Bind) -> Result<(), LoadConfigError> {
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

    pub async fn build(&self, path: PathBuf) -> Result<(), LoadConfigError> {
        let mut dockerfile = tokio::fs::File::create(path.join("Dockerfile")).await?;
        dockerfile
            .write_all(self.get_dockerfile()?.as_bytes())
            .await?;

        let mut named_conf_options =
            tokio::fs::File::create(path.join("named.conf.options")).await?;
        named_conf_options
            .write_all(self.get_named_conf_options()?.as_bytes())
            .await?;

        let mut named_conf_local = tokio::fs::File::create(path.join("named.conf.local")).await?;
        named_conf_local
            .write_all(self.get_named_conf_local()?.as_bytes())
            .await?;

        let mut named_conf_local = tokio::fs::File::create(path.join("named.conf.local")).await?;
        named_conf_local
            .write_all(self.get_named_conf_local()?.as_bytes())
            .await?;

        for (zone, config) in self.zones.iter() {
            let mut db_zone = tokio::fs::File::create(path.join(format!("db.{}", zone))).await?;
            db_zone
                .write_all(config.get_db_zone(zone)?.as_bytes())
                .await?;
        }

        Ok(())
    }

    fn get_dockerfile(&self) -> Result<String, LoadConfigError> {
        Ok(format!(
            r#"
FROM ubuntu/bind9:latest

COPY ./named.conf.local /etc/bind/named.conf.local
COPY ./named.conf.options /etc/bind/named.conf.options
COPY ./db.* /etc/bind/
"#
        ))
    }

    fn get_named_conf_options(&self) -> Result<String, LoadConfigError> {
        Ok(format!(
            r#"
options {{
	directory "/var/cache/bind";

	// If there is a firewall between you and nameservers you want
	// to talk to, you may need to fix the firewall to allow multiple
	// ports to talk.  See http://www.kb.cert.org/vuls/id/800113

	// If your ISP provided one or more IP addresses for stable 
	// nameservers, you probably want to use them as forwarders.  
	// Uncomment the following block, and insert the addresses replacing 
	// the all-0's placeholder.
    allow-query {{ any; }};

    recursion yes;
	forwarders {{
		8.8.8.8;
        8.8.4.4;
        1.1.1.1;
	}};

	//========================================================================
	// If BIND logs error messages about the root key being expired,
	// you will need to update your keys.  See https://www.isc.org/bind-keys
	//========================================================================
	dnssec-validation auto;

	auth-nxdomain no;    # conform to RFC1035
	listen-on-v6 {{ any; }};
}};
"#
        ))
    }

    fn get_named_conf_local(&self) -> Result<String, LoadConfigError> {
        let mut zones = String::new();
        for (zone, _) in self.zones.iter() {
            zones.push_str(&self.get_named_conf_local_zone(zone)?);
        }
        Ok(format!(
            r#"
//
// Do any local configuration here
//

// Consider adding the 1918 zones here, if they are not used in your
// organization
//include "/etc/bind/zones.rfc1918";
{zones}
"#,
            zones = zones
        ))
    }

    fn get_named_conf_local_zone(&self, zone: &str) -> Result<String, LoadConfigError> {
        Ok(format!(
            r#"
zone "{zone}" {{
        type master;
        file "/etc/bind/db.{zone}";
}};
"#,
            zone = zone
        ))
    }
}

impl ZoneBuilder {
    fn get_db_zone_nameserver(
        &self,
        nameserver: &str,
        zone: &str,
    ) -> Result<String, LoadConfigError> {
        Ok(format!(
            r#"
@         IN    NS      {nameserver}.{zone}.
"#,
            nameserver = nameserver,
            zone = zone
        ))
    }

    fn get_db_zone_a(&self, nameserver: &str, ip: &str) -> Result<String, LoadConfigError> {
        Ok(format!(
            r#"
{nameserver}       IN    A       {ip}
"#,
            nameserver = nameserver,
            ip = ip
        ))
    }

    fn get_db_zone_cname(&self, subdomain: &str, zone: &str) -> Result<String, LoadConfigError> {
        Ok(format!(
            r#"
{subdomain}.{zone}.       IN    CNAME       {zone}.
"#,
            subdomain = subdomain,
            zone = zone
        ))
    }

    fn get_db_zone(&self, zone: &str) -> Result<String, LoadConfigError> {
        let mut records = String::new();
        for (nameserver, ip) in self.nameservers.iter() {
            records.push_str(&self.get_db_zone_nameserver(nameserver, zone)?);
        }
        if let Some(ip) = self.ip.as_ref() {
            records.push_str(&self.get_db_zone_a("@", ip)?);
        }
        for (nameserver, ip) in self.nameservers.iter() {
            records.push_str(&self.get_db_zone_a(nameserver, ip)?);
        }
        for subdomain in self.cnames.iter() {
            records.push_str(&self.get_db_zone_cname(subdomain, zone)?);
        }
        Ok(format!(
            r#"
$TTL    10800
@       IN      SOA     {zone}. root.{zone}. (
                              7         ; Serial
                          10800         ; Refresh
                           3600         ; Retry
                        1209600         ; Expire
                           3600 )       ; Negative Cache TTL
;
{records}
"#,
            zone = zone,
            records = records
        ))
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
    pub async fn load<'a>(
        context: &super::LoadContext<'a>,
    ) -> Result<Option<Bind>, LoadConfigError> {
        raw::load(context).await
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::lib::{
        config::{self, LoadRawSync},
        utils,
    };

    #[derive(Serialize, Deserialize)]
    pub struct Config {
        bind9: Bind,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Bind {
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

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            Ok(super::Bind {
                zones: self.zones.load_raw(context)?,
            })
        }
    }

    impl config::LoadRawSync for Zone {
        type Output = super::Zone;

        fn load_raw(
            self,
            context: &config::LoadContext,
        ) -> Result<Self::Output, config::LoadConfigError> {
            Ok(super::Zone {
                ip: self.ip,
                nameservers: self.nameservers.unwrap_or_default(),
                cnames: self.cnames.unwrap_or_default(),
            })
        }
    }

    impl config::AutoLoadRaw for Config {}

    pub async fn load<'a>(
        context: &config::LoadContext<'a>,
    ) -> Result<Option<super::Bind>, super::LoadConfigError> {
        let path = context.project_config()?.clone();
        if path.exists() {
            Ok(Some(
                config::load_sync::<Config>(path, context)
                    .await?
                    .bind9
                    .load_raw(context)?,
            ))
        } else {
            Ok(None)
        }
    }
}
