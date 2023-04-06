// FIXME: This file is very personal, maybe make it more generic
use std::{collections::HashMap, path::PathBuf};

use super::LoadConfigError;

#[derive(Debug)]
pub struct Bind {
    zones: HashMap<String, Zone>,
}

#[derive(Debug)]
pub struct Zone {
    nameservers: HashMap<String, String>,
    cnames: Vec<String>,
}

impl Bind {
    pub async fn load<'a>(context: &super::LoadContext<'a>) -> Result<Bind, LoadConfigError> {
        raw::load(context).await
    }

    pub async fn build<'a>(path: PathBuf) -> Result<(), super::ExecutionError> {}
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::lib::{config, utils};

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Bind {
        zones: HashMap<String, Zone>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Zone {
        nameservers: HashMap<String, String>,
        cnames: Vec<String>,
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
                nameservers: self.nameservers,
                cnames: self.cnames,
            })
        }
    }

    pub async fn load<'a>(
        context: &config::LoadContext<'a>,
    ) -> Result<super::Bind, super::LoadConfigError> {
        config::load_sync::<Bind>(context.project_root()?.clone(), context).await
    }
}

const NAMED_CONF_OPTIONS: &str = r#"
options {
	directory "/var/cache/bind";

	// If there is a firewall between you and nameservers you want
	// to talk to, you may need to fix the firewall to allow multiple
	// ports to talk.  See http://www.kb.cert.org/vuls/id/800113

	// If your ISP provided one or more IP addresses for stable 
	// nameservers, you probably want to use them as forwarders.  
	// Uncomment the following block, and insert the addresses replacing 
	// the all-0's placeholder.
    allow-query { any; };

    recursion yes;
	forwarders {
		8.8.8.8;
        8.8.4.4;
        1.1.1.1;
	};

	//========================================================================
	// If BIND logs error messages about the root key being expired,
	// you will need to update your keys.  See https://www.isc.org/bind-keys
	//========================================================================
	dnssec-validation auto;

	auth-nxdomain no;    # conform to RFC1035
	listen-on-v6 { any; };
};
"#;


const NAMED_CONF_LOCAL_HEADER: &str = r#"
//
// Do any local configuration here
//

// Consider adding the 1918 zones here, if they are not used in your
// organization
//include "/etc/bind/zones.rfc1918";
"#;

const NAMED_CONF_LOCAL_ZONE: &str = r#"
zone "${zone}" {
        type master;
        file "/etc/bind/db.${zone}";
};
"#;
