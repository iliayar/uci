
use std::{path::PathBuf, collections::{HashMap, HashSet}};

use tokio::io::AsyncWriteExt;

use crate::lib::config;

pub struct GenBind {
    pub zones: HashMap<String, GenZone>,
}

pub struct GenZone {
    pub ip: Option<String>,
    pub nameservers: HashMap<String, String>,
    pub cnames: HashSet<String>,
}

impl GenBind {
    pub fn is_empty(&self) -> bool {
	self.zones.is_empty()
    }

    pub async fn gen(&self, path: PathBuf) -> Result<(), super::CodegenError> {
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

    fn get_dockerfile(&self) -> Result<String, super::CodegenError> {
        Ok(format!(
            r#"
FROM ubuntu/bind9:latest

COPY ./named.conf.local /etc/bind/named.conf.local
COPY ./named.conf.options /etc/bind/named.conf.options
COPY ./db.* /etc/bind/
"#
        ))
    }

    fn get_named_conf_options(&self) -> Result<String, super::CodegenError> {
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

    fn get_named_conf_local(&self) -> Result<String, super::CodegenError> {
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

    fn get_named_conf_local_zone(&self, zone: &str) -> Result<String, super::CodegenError> {
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

impl GenZone {
    fn get_db_zone_nameserver(
        &self,
        nameserver: &str,
        zone: &str,
    ) -> Result<String, super::CodegenError> {
        Ok(format!(
            r#"
@         IN    NS      {nameserver}.{zone}.
"#,
            nameserver = nameserver,
            zone = zone
        ))
    }

    fn get_db_zone_a(&self, nameserver: &str, ip: &str) -> Result<String, super::CodegenError> {
        Ok(format!(
            r#"
{nameserver}       IN    A       {ip}
"#,
            nameserver = nameserver,
            ip = ip
        ))
    }

    fn get_db_zone_cname(&self, subdomain: &str, zone: &str) -> Result<String, super::CodegenError> {
        Ok(format!(
            r#"
{subdomain}.{zone}.       IN    CNAME       {zone}.
"#,
            subdomain = subdomain,
            zone = zone
        ))
    }

    fn get_db_zone(&self, zone: &str) -> Result<String, super::CodegenError> {
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
