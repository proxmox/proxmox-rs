use serde::{Deserialize, Serialize};

use proxmox_schema::api;
use proxmox_schema::api_types::IP_FORMAT;
use proxmox_schema::Schema;
use proxmox_schema::StringSchema;

use proxmox_product_config::ConfigDigest;

pub const SEARCH_DOMAIN_SCHEMA: Schema =
    StringSchema::new("Search domain for host-name lookup.").schema();

pub const FIRST_DNS_SERVER_SCHEMA: Schema = StringSchema::new("First name server IP address.")
    .format(&IP_FORMAT)
    .schema();

pub const SECOND_DNS_SERVER_SCHEMA: Schema = StringSchema::new("Second name server IP address.")
    .format(&IP_FORMAT)
    .schema();

pub const THIRD_DNS_SERVER_SCHEMA: Schema = StringSchema::new("Third name server IP address.")
    .format(&IP_FORMAT)
    .schema();

#[api(
    properties: {
        search: {
            schema: SEARCH_DOMAIN_SCHEMA,
            optional: true,
        },
        dns1: {
            optional: true,
            schema: FIRST_DNS_SERVER_SCHEMA,
        },
        dns2: {
            optional: true,
            schema: SECOND_DNS_SERVER_SCHEMA,
        },
        dns3: {
            optional: true,
            schema: THIRD_DNS_SERVER_SCHEMA,
        },
        options: {
            description: "Other data found in the configuration file (resolv.conf).",
            optional: true,
        },

    }
)]
#[derive(Serialize, Deserialize, Default)]
/// DNS configuration from '/etc/resolv.conf'
pub struct ResolvConf {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns3: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<String>,
}

#[api(
    properties: {
        config: {
            type: ResolvConf,
        },
        digest: {
            type: ConfigDigest,
        },
    }
)]
#[derive(Serialize, Deserialize)]
/// DNS configuration with digest.
pub struct ResolvConfWithDigest {
    #[serde(flatten)]
    pub config: ResolvConf,
    pub digest: ConfigDigest,
}


#[api()]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Deletable DNS configuration property name
pub enum DeletableResolvConfProperty {
    /// Delete first nameserver entry
    Dns1,
    /// Delete second nameserver entry
    Dns2,
    /// Delete third nameserver entry
    Dns3,
}
