use anyhow::bail;
use const_format::concatcp;
use serde::{Deserialize, Serialize};

use proxmox_schema::api_types::{
    CERT_FINGERPRINT_SHA256_SCHEMA, DNS_LABEL_STR, IPRE_STR, SAFE_ID_FORMAT,
};
use proxmox_schema::{api, const_regex, ApiStringFormat, Schema, StringSchema, Updater};

#[rustfmt::skip]
/// Regex to match S3 endpoint full qualified domain names, including template patterns for bucket
/// name or region.
pub const S3_ENDPOINT_NAME_STR: &str = concatcp!(
    r"(^\{\{bucket\}\}\.)*(?:(?:(", DNS_LABEL_STR, r"|\{\{region\}\})\.)*", DNS_LABEL_STR, ")"
);

const_regex! {
    /// Regex to match S3 bucket names.
    ///
    /// Be as strict as possible following the rules as described here:
    /// https://docs.aws.amazon.com/AmazonS3/latest/userguide/bucketnamingrules.html#general-purpose-bucket-names
    pub S3_BUCKET_NAME_REGEX = r"^[a-z0-9]([a-z0-9\-]*[a-z0-9])?$";
    /// Regex to match S3 endpoints including template patterns.
    pub S3_ENDPOINT_REGEX = concatcp!(r"^(?:", S3_ENDPOINT_NAME_STR, "|",  IPRE_STR, r")$");
    /// Regex to match S3 regions, similar to SAFE_ID_REGEX but only lower case and without dot.
    pub S3_REGION_REGEX = r"^[_a-z\d][-_a-z\d]+$";
}

/// S3 REST API endpoint format.
pub const S3_ENDPOINT_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&S3_ENDPOINT_REGEX);
/// S3 region format.
pub const S3_REGION_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&S3_REGION_REGEX);

/// ID to uniquely identify an S3 client config.
pub const S3_CLIENT_ID_SCHEMA: Schema =
    StringSchema::new("ID to uniquely identify s3 client config.")
        .format(&SAFE_ID_FORMAT)
        .min_length(3)
        .max_length(32)
        .schema();

/// Endpoint to access S3 object store.
pub const S3_ENDPOINT_SCHEMA: Schema = StringSchema::new("Endpoint to access S3 object store.")
    .format(&S3_ENDPOINT_FORMAT)
    .schema();

/// Region to access S3 object store.
pub const S3_REGION_SCHEMA: Schema = StringSchema::new("Region to access S3 object store.")
    .format(&S3_REGION_FORMAT)
    .min_length(3)
    .max_length(32)
    .schema();

/// Bucket to access S3 object store.
pub const S3_BUCKET_NAME_SCHEMA: Schema = StringSchema::new("Bucket name for S3 object store.")
    .format(&ApiStringFormat::VerifyFn(|bucket_name| {
        if !(S3_BUCKET_NAME_REGEX.regex_obj)().is_match(bucket_name) {
            bail!("Bucket name does not match the regex pattern");
        }

        // Exclude pre- and postfixes described here:
        // https://docs.aws.amazon.com/AmazonS3/latest/userguide/bucketnamingrules.html#general-purpose-bucket-names
        let forbidden_prefixes = ["xn--", "sthree-", "amzn-s3-demo-"];
        for prefix in forbidden_prefixes {
            if bucket_name.starts_with(prefix) {
                bail!("Bucket name cannot start with '{prefix}'");
            }
        }

        let forbidden_postfixes = ["--ol-s3", ".mrap", "--x-s3"];
        for postfix in forbidden_postfixes {
            if bucket_name.ends_with(postfix) {
                bail!("Bucket name cannot end with '{postfix}'");
            }
        }

        Ok(())
    }))
    .min_length(3)
    .max_length(63)
    .schema();

#[api]
#[derive(Copy, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
/// Provider specific feature implementation quirks.
pub enum ProviderQuirks {
    /// Prvider does not support the If-None-Match http header
    SkipIfNoneMatchHeader,
}
serde_plain::derive_display_from_serialize!(ProviderQuirks);
serde_plain::derive_fromstr_from_deserialize!(ProviderQuirks);

#[api(
    properties: {
        endpoint: {
            schema: S3_ENDPOINT_SCHEMA,
        },
        port: {
            type: u16,
            optional: true,
        },
        region: {
            schema: S3_REGION_SCHEMA,
            optional: true,
        },
        fingerprint: {
            schema: CERT_FINGERPRINT_SHA256_SCHEMA,
            optional: true,
        },
        "access-key": {
            type: String,
        },
        "path-style": {
            type: bool,
            optional: true,
            default: false,
        },
        "put-rate-limit": {
            type: u64,
            optional: true,
        },
        "provider-quirks": {
            type: Array,
            optional: true,
            items: {
                type: ProviderQuirks,
            },
        },
    },
)]
#[derive(Serialize, Deserialize, Updater, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// S3 client configuration properties.
pub struct S3ClientConfig {
    /// Endpoint to access S3 object store.
    pub endpoint: String,
    /// Port to access S3 object store.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Region to access S3 object store.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// Access key for S3 object store.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// Access key for S3 object store.
    pub access_key: String,
    /// Use path style bucket addressing over vhost style.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_style: Option<bool>,
    /// Rate limit for put requests given as #reqest/s.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub put_rate_limit: Option<u64>,
    /// List of provider specific feature implementation quirks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_quirks: Option<Vec<ProviderQuirks>>,
}

impl S3ClientConfig {
    /// Helper method to get ACL path for S3 client config
    pub fn acl_path(&self) -> Vec<&str> {
        // Needs permissions on root path
        Vec::new()
    }
}

#[api(
    properties: {
        id: {
            schema: S3_CLIENT_ID_SCHEMA,
        },
        config: {
            type: S3ClientConfig,
        },
        "secret-key": {
            type: String,
        },
    },
)]
#[derive(Serialize, Deserialize, Updater, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// S3 client configuration.
pub struct S3ClientConf {
    /// ID to identify s3 client config.
    #[updater(skip)]
    pub id: String,
    /// S3 client config.
    #[serde(flatten)]
    pub config: S3ClientConfig,
    /// Secret key for S3 object store.
    pub secret_key: String,
}

#[api(
    properties: {
        id: {
            schema: S3_CLIENT_ID_SCHEMA,
        },
        config: {
            type: S3ClientConfig,
        },
    },
)]
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// S3 client configuration properties without secret.
pub struct S3ClientConfigWithoutSecret {
    /// ID to identify s3 client config.
    pub id: String,
    /// S3 client config.
    #[serde(flatten)]
    pub config: S3ClientConfig,
}

#[api(
    properties: {
        name: {
            schema: S3_BUCKET_NAME_SCHEMA,
        },
    },
)]
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// S3 bucket list item.
pub struct S3BucketListItem {
    /// S3 bucket name.
    pub name: String,
}
