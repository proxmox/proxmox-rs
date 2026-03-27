use std::ffi::OsStr;

use anyhow::Error;
use serde::{Deserialize, Serialize};

use proxmox_acme_api::{AcmeConfig, AcmeDomain, ACME_DOMAIN_PROPERTY_SCHEMA};
use proxmox_auth_api::types::Authid;
#[cfg(feature = "enum-fallback")]
use proxmox_fixed_string::FixedString;
use proxmox_http::{ProxyConfig, HTTP_PROXY_SCHEMA};
use proxmox_schema::*;

use crate::{
    StorageStatus, EMAIL_SCHEMA, MULTI_LINE_COMMENT_SCHEMA, OPENSSL_CIPHERS_TLS_1_2_SCHEMA,
    OPENSSL_CIPHERS_TLS_1_3_SCHEMA,
};

#[api]
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
/// Node memory usage counters
pub struct NodeMemoryCounters {
    /// Total memory
    pub total: u64,
    /// Used memory
    pub used: u64,
    /// Free memory
    pub free: u64,
}

#[api]
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
/// Node swap usage counters
pub struct NodeSwapCounters {
    /// Total swap
    pub total: u64,
    /// Used swap
    pub used: u64,
    /// Free swap
    pub free: u64,
}

#[api]
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
/// Contains general node information such as the fingerprint`
pub struct NodeInformation {
    /// The SSL Fingerprint
    pub fingerprint: String,
}

#[api]
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
/// The current kernel version (output of `uname`)
pub struct KernelVersionInformation {
    /// The systemname/nodename
    pub sysname: String,
    /// The kernel release number
    pub release: String,
    /// The kernel version
    pub version: String,
    /// The machine architecture
    pub machine: String,
}

impl KernelVersionInformation {
    pub fn from_uname_parts(
        sysname: &OsStr,
        release: &OsStr,
        version: &OsStr,
        machine: &OsStr,
    ) -> Self {
        KernelVersionInformation {
            sysname: sysname.to_str().map(String::from).unwrap_or_default(),
            release: release.to_str().map(String::from).unwrap_or_default(),
            version: version.to_str().map(String::from).unwrap_or_default(),
            machine: machine.to_str().map(String::from).unwrap_or_default(),
        }
    }

    pub fn get_legacy(&self) -> String {
        format!("{} {} {}", self.sysname, self.release, self.version)
    }
}

#[api]
#[derive(Serialize, Deserialize, Copy, Clone)]
#[serde(rename_all = "kebab-case")]
/// The possible BootModes
pub enum BootMode {
    /// The BootMode is EFI/UEFI
    Efi,
    /// The BootMode is Legacy BIOS
    LegacyBios,
    #[cfg(feature = "enum-fallback")]
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}

#[api]
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
/// Holds the Bootmodes
pub struct BootModeInformation {
    /// The BootMode, either Efi or Bios
    pub mode: BootMode,
    /// SecureBoot status
    pub secureboot: bool,
}

#[api]
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
/// Information about the CPU
pub struct NodeCpuInformation {
    /// The CPU model
    pub model: String,
    /// The number of CPU sockets
    pub sockets: usize,
    /// The number of CPU cores (incl. threads)
    pub cpus: usize,
}

#[api(
    properties: {
        memory: {
            type: NodeMemoryCounters,
        },
        root: {
            type: StorageStatus,
        },
        swap: {
            type: NodeSwapCounters,
        },
        loadavg: {
            type: Array,
            items: {
                type: Number,
                description: "the load",
            }
        },
        cpuinfo: {
            type: NodeCpuInformation,
        },
        info: {
            type: NodeInformation,
        }
    },
)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// The Node status
pub struct NodeStatus {
    pub memory: NodeMemoryCounters,
    pub root: StorageStatus,
    pub swap: NodeSwapCounters,
    /// The current uptime of the server.
    pub uptime: u64,
    /// Load for 1, 5 and 15 minutes.
    pub loadavg: [f64; 3],
    /// The current kernel version (NEW struct type).
    pub current_kernel: KernelVersionInformation,
    /// The current kernel version (LEGACY string type).
    pub kversion: String,
    /// Total CPU usage since last query.
    pub cpu: f64,
    /// Total IO wait since last query.
    pub wait: f64,
    pub cpuinfo: NodeCpuInformation,
    pub info: NodeInformation,
    /// Current boot mode
    pub boot_info: BootModeInformation,
}

#[api(
    properties: {
        port: {
            type: Integer,
        },
        ticket: {
            type: String,
        },
        upid: {
            type: String,
        },
        user: {
            type: String,
        },
    },
)]
/// Ticket used for authenticating a VNC websocket upgrade request.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct NodeShellTicket {
    /// port used to bind termproxy to
    pub port: u16,

    /// ticket used to verifiy websocket connection
    pub ticket: String,

    /// UPID for termproxy worker task
    pub upid: String,

    /// user or authid encoded in the ticket
    pub user: Authid,
}

/// All available languages in Proxmox. Taken from proxmox-i18n repository.
/// pt_BR, zh_CN, and zh_TW use the same case in the translation files.
// TODO: auto-generate from available translations
#[api]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Translation {
    /// Arabic
    Ar,
    /// Catalan
    Ca,
    /// Danish
    Da,
    /// German
    De,
    /// English
    En,
    /// Spanish
    Es,
    /// Euskera
    Eu,
    /// Persian (Farsi)
    Fa,
    /// French
    Fr,
    /// Galician
    Gl,
    /// Hebrew
    He,
    /// Hungarian
    Hu,
    /// Italian
    It,
    /// Japanese
    Ja,
    /// Korean
    Kr,
    /// Norwegian (Bokmal)
    Nb,
    /// Dutch
    Nl,
    /// Norwegian (Nynorsk)
    Nn,
    /// Polish
    Pl,
    /// Portuguese (Brazil)
    #[serde(rename = "pt_BR")]
    PtBr,
    /// Russian
    Ru,
    /// Slovenian
    Sl,
    /// Swedish
    Sv,
    /// Turkish
    Tr,
    /// Chinese (simplified)
    #[serde(rename = "zh_CN")]
    ZhCn,
    /// Chinese (traditional)
    #[serde(rename = "zh_TW")]
    ZhTw,
}

#[api(
    properties: {
        acme: {
            optional: true,
            type: String,
            format: &ApiStringFormat::PropertyString(&AcmeConfig::API_SCHEMA),
        },
        acmedomain0: {
            schema: ACME_DOMAIN_PROPERTY_SCHEMA,
            optional: true,
        },
        acmedomain1: {
            schema: ACME_DOMAIN_PROPERTY_SCHEMA,
            optional: true,
        },
        acmedomain2: {
            schema: ACME_DOMAIN_PROPERTY_SCHEMA,
            optional: true,
        },
        acmedomain3: {
            schema: ACME_DOMAIN_PROPERTY_SCHEMA,
            optional: true,
        },
        acmedomain4: {
            schema: ACME_DOMAIN_PROPERTY_SCHEMA,
            optional: true,
        },
        "http-proxy": {
            schema: HTTP_PROXY_SCHEMA,
            optional: true,
        },
        "email-from": {
            schema: EMAIL_SCHEMA,
            optional: true,
        },
        "ciphers-tls-1.3": {
            schema: OPENSSL_CIPHERS_TLS_1_3_SCHEMA,
            optional: true,
        },
        "ciphers-tls-1.2": {
            schema: OPENSSL_CIPHERS_TLS_1_2_SCHEMA,
            optional: true,
        },
        "default-lang" : {
            schema: Translation::API_SCHEMA,
            optional: true,
        },
        "description" : {
            optional: true,
            schema: MULTI_LINE_COMMENT_SCHEMA,
        },
        "consent-text" : {
            optional: true,
            type: String,
            max_length: 64 * 1024,
        }
    },
)]
#[derive(Deserialize, Serialize, Updater)]
#[serde(rename_all = "kebab-case")]
/// Node specific configuration.
pub struct NodeConfig {
    /// The acme account to use on this node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acme: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub acmedomain0: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub acmedomain1: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub acmedomain2: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub acmedomain3: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub acmedomain4: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_proxy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_from: Option<String>,

    /// List of TLS ciphers for TLS 1.3 that will be used by the proxy. (Proxy has to be restarted for changes to take effect)
    #[serde(skip_serializing_if = "Option::is_none", rename = "ciphers-tls-1.3")]
    pub ciphers_tls_1_3: Option<String>,

    /// List of TLS ciphers for TLS <= 1.2 that will be used by the proxy. (Proxy has to be restarted for changes to take effect)
    #[serde(skip_serializing_if = "Option::is_none", rename = "ciphers-tls-1.2")]
    pub ciphers_tls_1_2: Option<String>,

    /// Default language used in the GUI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_lang: Option<String>,

    /// Node description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Maximum days to keep Task logs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_log_max_days: Option<usize>,

    /// Consent banner text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consent_text: Option<String>,
}

impl NodeConfig {
    pub fn acme_config(&self) -> Result<AcmeConfig, Error> {
        self.acme
            .as_deref()
            .map(|config| {
                Ok(serde_json::from_value(
                    AcmeConfig::API_SCHEMA.parse_property_string(config)?,
                )?)
            })
            .unwrap_or_else(|| proxmox_acme_api::parse_acme_config_string("account=default"))
    }

    pub fn acme_domains(&'_ self) -> AcmeDomainIter<'_> {
        AcmeDomainIter::new(self)
    }

    /// Returns the parsed ProxyConfig
    pub fn http_proxy(&self) -> Option<ProxyConfig> {
        if let Some(http_proxy) = &self.http_proxy {
            ProxyConfig::parse_proxy_url(http_proxy).ok()
        } else {
            None
        }
    }

    /// Sets the HTTP proxy configuration
    pub fn set_http_proxy(&mut self, http_proxy: Option<String>) {
        self.http_proxy = http_proxy;
    }
}

pub struct AcmeDomainIter<'a> {
    config: &'a NodeConfig,
    index: usize,
}

impl<'a> AcmeDomainIter<'a> {
    fn new(config: &'a NodeConfig) -> Self {
        Self { config, index: 0 }
    }
}

impl Iterator for AcmeDomainIter<'_> {
    type Item = Result<AcmeDomain, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let domain = loop {
            let index = self.index;
            self.index += 1;

            let domain = match index {
                0 => self.config.acmedomain0.as_deref(),
                1 => self.config.acmedomain1.as_deref(),
                2 => self.config.acmedomain2.as_deref(),
                3 => self.config.acmedomain3.as_deref(),
                4 => self.config.acmedomain4.as_deref(),
                _ => return None,
            };

            if let Some(domain) = domain {
                break domain;
            }
        };

        Some(
            AcmeDomain::API_SCHEMA
                .parse_property_string(domain)
                .and_then(|domain| Ok(serde_json::from_value(domain)?)),
        )
    }
}
