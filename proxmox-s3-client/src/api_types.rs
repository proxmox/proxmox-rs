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
    r"(?:(?:(", DNS_LABEL_STR, r"|\{\{bucket\}\}|\{\{region\}\})\.)*", DNS_LABEL_STR, ")"
);

const_regex! {
    /// Regex to match S3 bucket names.
    ///
    /// Be as strict as possible following the rules as described here:
    /// https://docs.aws.amazon.com/AmazonS3/latest/userguide/bucketnamingrules.html#general-purpose-bucket-names
    pub S3_BUCKET_NAME_REGEX = r"^[a-z0-9]([a-z0-9\-]*[a-z0-9])?$";
    /// Regex to match S3 endpoints including template patterns.
    pub S3_ENDPOINT_REGEX = concatcp!(r"^(?:", S3_ENDPOINT_NAME_STR, "|",  IPRE_STR, r")$");
    /// Regex to match S3 regions.
    pub S3_REGION_REGEX = r"(^auto$)|(^[a-z]{2,}(?:-[a-z\d]+)+$)";
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

#[api(
    properties: {
        id: {
            schema: S3_CLIENT_ID_SCHEMA,
        },
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
    }
)]
#[derive(Serialize, Deserialize, Updater, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// S3 client configuration properties.
pub struct S3ClientConfig {
    /// ID to identify s3 client config.
    #[updater(skip)]
    pub id: String,
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
        "secrets-id": {
            type: String,
        },
        "secret-key": {
            type: String,
        },
    }
)]
#[derive(Serialize, Deserialize, Updater, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// S3 client secrets configuration properties.
pub struct S3ClientSecretsConfig {
    /// ID to identify s3 client secret config.
    #[updater(skip)]
    pub secrets_id: String,
    /// Secret key for S3 object store.
    pub secret_key: String,
}
