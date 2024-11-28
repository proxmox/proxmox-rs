//! Defines the types for the api version info endpoint
use std::convert::TryFrom;

use anyhow::{format_err, Context};

use proxmox_schema::api;

#[api(
    description: "Api version information",
    properties: {
        "version": {
            description: "Version 'major.minor'",
            type: String,
        },
        "release": {
            description: "Version release",
            type: String,
        },
        "repoid": {
            description: "Version repository id",
            type: String,
        },
    }
)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct ApiVersionInfo {
    pub version: String,
    pub release: String,
    pub repoid: String,
}

pub type ApiVersionMajor = u64;
pub type ApiVersionMinor = u64;
pub type ApiVersionRelease = u64;

pub struct ApiVersion {
    pub major: ApiVersionMajor,
    pub minor: ApiVersionMinor,
    pub release: ApiVersionRelease,
}

impl TryFrom<ApiVersionInfo> for ApiVersion {
    type Error = anyhow::Error;

    fn try_from(value: ApiVersionInfo) -> Result<Self, Self::Error> {
        let (major, minor) = value
            .version
            .split_once('.')
            .ok_or_else(|| format_err!("malformed API version {}", value.version))?;

        let major: ApiVersionMajor = major
            .parse()
            .with_context(|| "failed to parse major version")?;
        let minor: ApiVersionMinor = minor
            .parse()
            .with_context(|| "failed to parse minor version")?;
        let release: ApiVersionRelease = value
            .release
            .parse()
            .with_context(|| "failed to parse release version")?;

        Ok(Self {
            major,
            minor,
            release,
        })
    }
}
