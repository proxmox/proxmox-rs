//! ACME API type definitions.

use std::borrow::Cow;

use anyhow::Error;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use proxmox_schema::{api, ApiStringFormat, ApiType, Schema, StringSchema, Updater};
use proxmox_schema::api_types::{DNS_ALIAS_FORMAT, DNS_NAME_FORMAT, SAFE_ID_FORMAT};

use proxmox_acme::types::AccountData as AcmeAccountData;

#[api(
    properties: {
        san: {
            type: Array,
            items: {
                description: "A SubjectAlternateName entry.",
                type: String,
            },
        },
    },
)]
/// Certificate information.
#[derive(PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct CertificateInfo {
    /// Certificate file name.
    pub filename: String,

    /// Certificate subject name.
    pub subject: String,

    /// List of certificate's SubjectAlternativeName entries.
    pub san: Vec<String>,

    /// Certificate issuer name.
    pub issuer: String,

    /// Certificate's notBefore timestamp (UNIX epoch).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notbefore: Option<i64>,

    /// Certificate's notAfter timestamp (UNIX epoch).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notafter: Option<i64>,

    /// Certificate in PEM format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pem: Option<String>,

    /// Certificate's public key algorithm.
    pub public_key_type: String,

    /// Certificate's public key size if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key_bits: Option<u32>,

    /// The SSL Fingerprint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
}

proxmox_schema::api_string_type! {
    #[api(format: &SAFE_ID_FORMAT)]
    /// ACME account name.
    #[derive(Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
    #[serde(transparent)]
    pub struct AcmeAccountName(String);
}

#[api(
    properties: {
        name: { type: String },
        url: { type: String },
    },
)]
/// An ACME directory endpoint with a name and URL.
#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct KnownAcmeDirectory {
    /// The ACME directory's name.
    pub name: Cow<'static, str>,
    /// The ACME directory's endpoint URL.
    pub url: Cow<'static, str>,
}

#[api(
    properties: {
        schema: {
            type: Object,
            additional_properties: true,
            properties: {},
        },
        type: {
            type: String,
        },
    },
)]
#[derive(Clone, Deserialize, Serialize, PartialEq)]
/// Schema for an ACME challenge plugin.
pub struct AcmeChallengeSchema {
    /// Plugin ID.
    pub id: String,

    /// Human readable name, falls back to id.
    pub name: String,

    /// Plugin Type.
    #[serde(rename = "type")]
    pub ty: String,

    /// The plugin's parameter schema.
    pub schema: Value,
}

#[api(
    properties: {
        "domain": { format: &DNS_NAME_FORMAT },
        "alias": {
            optional: true,
            format: &DNS_ALIAS_FORMAT,
        },
        "plugin": {
            optional: true,
            format: &SAFE_ID_FORMAT,
        },
    },
    default_key: "domain",
)]
#[derive(Clone, PartialEq, Deserialize, Serialize)]
/// A domain entry for an ACME certificate.
pub struct AcmeDomain {
    /// The domain to certify for.
    pub domain: String,

    /// The domain to use for challenges instead of the default acme challenge domain.
    ///
    /// This is useful if you use CNAME entries to redirect `_acme-challenge.*` domains to a
    /// different DNS server.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,

    /// The plugin to use to validate this domain.
    ///
    /// Empty means standalone HTTP validation is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
}

/// ACME domain configuration string [Schema].
pub const ACME_DOMAIN_PROPERTY_SCHEMA: Schema =
    StringSchema::new("ACME domain configuration string")
        .format(&ApiStringFormat::PropertyString(&AcmeDomain::API_SCHEMA))
        .schema();

/// Parse [AcmeDomain] from property string.
pub fn parse_acme_domain_string(value_str: &str) -> Result<AcmeDomain, Error> {
    let value = AcmeDomain::API_SCHEMA.parse_property_string(value_str)?;
    let value: AcmeDomain = serde_json::from_value(value)?;
    Ok(value)
}

/// Format [AcmeDomain] as property string.
pub fn create_acme_domain_string(config: &AcmeDomain) -> String {
    proxmox_schema::property_string::print::<AcmeDomain>(config).unwrap()
}

#[api()]
#[derive(Clone, PartialEq, Deserialize, Serialize)]
/// ACME Account information.
///
/// This is what we return via the API.
pub struct AccountInfo {
    /// Raw account data.
    pub account: AcmeAccountData,

    /// The ACME directory URL the account was created at.
    pub directory: String,

    /// The account's own URL within the ACME directory.
    pub location: String,

    /// The ToS URL, if the user agreed to one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tos: Option<String>,
}

/// An ACME Account entry.
///
/// Currently only contains a 'name' property.
#[api()]
#[derive(Clone, PartialEq, Deserialize, Serialize)]
pub struct AcmeAccountEntry {
    pub name: AcmeAccountName,
}

#[api()]
#[derive(Clone, PartialEq, Deserialize, Serialize)]
/// The ACME configuration.
///
/// Currently only contains the name of the account use.
pub struct AcmeConfig {
    /// Account to use to acquire ACME certificates.
    pub account: String,
}

/// Parse [AcmeConfig] from property string.
pub fn parse_acme_config_string(value_str: &str) -> Result<AcmeConfig, Error> {
    let value = AcmeConfig::API_SCHEMA.parse_property_string(value_str)?;
    let value: AcmeConfig = serde_json::from_value(value)?;
    Ok(value)
}

/// Format [AcmeConfig] as property string.
pub fn create_acme_config_string(config: &AcmeConfig) -> String {
    proxmox_schema::property_string::print::<AcmeConfig>(config).unwrap()
}

/// [Schema] for ACME Challenge Plugin ID.
pub const PLUGIN_ID_SCHEMA: Schema = StringSchema::new("ACME Challenge Plugin ID.")
    .format(&SAFE_ID_FORMAT)
    .min_length(1)
    .max_length(32)
    .schema();

#[api]
#[derive(Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
/// ACME plugin config. The API's format is inherited from PVE/PMG:
pub struct PluginConfig {
    /// Plugin ID.
    pub plugin: String,

    /// Plugin type.
    #[serde(rename = "type")]
    pub ty: String,

    /// DNS Api name.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub api: Option<String>,

    /// Plugin configuration data.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub data: Option<String>,

    /// Extra delay in seconds to wait before requesting validation.
    ///
    /// Allows to cope with long TTL of DNS records.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub validation_delay: Option<u32>,

    /// Flag to disable the config.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub disable: Option<bool>,
}

#[api(
    properties: {
        id: { schema: PLUGIN_ID_SCHEMA },
    },
)]
#[derive(Deserialize, Serialize)]
/// Standalone ACME Plugin for the http-1 challenge.
pub struct StandalonePlugin {
    /// Plugin ID.
    id: String,
}

impl Default for StandalonePlugin {
    fn default() -> Self {
        Self {
            id: "standalone".to_string(),
        }
    }
}

#[api(
    properties: {
        id: { schema: PLUGIN_ID_SCHEMA },
        disable: {
            optional: true,
            default: false,
        },
        "validation-delay": {
            default: 30,
            optional: true,
            minimum: 0,
            maximum: 2 * 24 * 60 * 60,
        },
    },
)]
/// DNS ACME Challenge Plugin core data.
#[derive(Deserialize, Serialize, Updater)]
#[serde(rename_all = "kebab-case")]
pub struct DnsPluginCore {
    /// Plugin ID.
    #[updater(skip)]
    pub id: String,

    /// DNS API Plugin Id.
    pub api: String,

    /// Extra delay in seconds to wait before requesting validation.
    ///
    /// Allows to cope with long TTL of DNS records.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub validation_delay: Option<u32>,

    /// Flag to disable the config.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub disable: Option<bool>,
}

#[api(
    properties: {
        core: { type: DnsPluginCore },
    },
)]
/// DNS ACME Challenge Plugin.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct DnsPlugin {
    #[serde(flatten)]
    pub core: DnsPluginCore,

    // We handle this property separately in the API calls.
    /// DNS plugin data (base64url encoded without padding).
    #[serde(with = "proxmox_serde::string_as_base64url_nopad")]
    pub data: String,
}

#[api()]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Deletable plugin property names.
pub enum DeletablePluginProperty {
    /// Delete the disable property
    Disable,
    /// Delete the validation-delay property
    ValidationDelay,
}

#[api(
    properties: {
        name: { type: AcmeAccountName },
    },
)]
/// An ACME Account entry.
///
/// Currently only contains a 'name' property.
#[derive(Clone, PartialEq, Deserialize, Serialize)]
pub struct AccountEntry {
    pub name: AcmeAccountName,
}
