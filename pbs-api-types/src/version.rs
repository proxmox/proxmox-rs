//! Defines the types for the api version info endpoint
use std::cmp::Ordering;
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

#[derive(PartialEq, Eq)]
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

impl PartialOrd for ApiVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let ordering = match (
            self.major.cmp(&other.major),
            self.minor.cmp(&other.minor),
            self.release.cmp(&other.release),
        ) {
            (Ordering::Equal, Ordering::Equal, ordering) => ordering,
            (Ordering::Equal, ordering, _) => ordering,
            (ordering, _, _) => ordering,
        };

        Some(ordering)
    }
}

impl ApiVersion {
    pub fn new(major: ApiVersionMajor, minor: ApiVersionMinor, release: ApiVersionRelease) -> Self {
        Self {
            major,
            minor,
            release,
        }
    }
}

#[test]
fn same_level_version_comarison() {
    let major_base = ApiVersion::new(2, 0, 0);
    let major_less = ApiVersion::new(1, 0, 0);
    let major_greater = ApiVersion::new(3, 0, 0);

    let minor_base = ApiVersion::new(2, 2, 0);
    let minor_less = ApiVersion::new(2, 1, 0);
    let minor_greater = ApiVersion::new(2, 3, 0);

    let release_base = ApiVersion::new(2, 2, 2);
    let release_less = ApiVersion::new(2, 2, 1);
    let release_greater = ApiVersion::new(2, 2, 3);

    assert!(major_base == major_base);
    assert!(minor_base == minor_base);
    assert!(release_base == release_base);

    assert!(major_base > major_less);
    assert!(major_base >= major_less);
    assert!(major_base != major_less);

    assert!(major_base < major_greater);
    assert!(major_base <= major_greater);
    assert!(major_base != major_greater);

    assert!(minor_base > minor_less);
    assert!(minor_base >= minor_less);
    assert!(minor_base != minor_less);

    assert!(minor_base < minor_greater);
    assert!(minor_base <= minor_greater);
    assert!(minor_base != minor_greater);

    assert!(release_base > release_less);
    assert!(release_base >= release_less);
    assert!(release_base != release_less);

    assert!(release_base < release_greater);
    assert!(release_base <= release_greater);
    assert!(release_base != release_greater);
}

#[test]
fn mixed_level_version_comarison() {
    let major_base = ApiVersion::new(2, 0, 0);
    let major_less = ApiVersion::new(1, 0, 0);
    let major_greater = ApiVersion::new(3, 0, 0);

    let minor_base = ApiVersion::new(2, 2, 0);
    let minor_less = ApiVersion::new(2, 1, 0);
    let minor_greater = ApiVersion::new(2, 3, 0);

    let release_base = ApiVersion::new(2, 2, 2);
    let release_less = ApiVersion::new(2, 2, 1);
    let release_greater = ApiVersion::new(2, 2, 3);

    assert!(major_base < minor_base);
    assert!(major_base < minor_less);
    assert!(major_base < minor_greater);

    assert!(major_base < release_base);
    assert!(major_base < release_less);
    assert!(major_base < release_greater);

    assert!(major_less < minor_base);
    assert!(major_less < minor_less);
    assert!(major_less < minor_greater);

    assert!(major_less < release_base);
    assert!(major_less < release_less);
    assert!(major_less < release_greater);

    assert!(major_greater > minor_base);
    assert!(major_greater > minor_less);
    assert!(major_greater > minor_greater);

    assert!(major_greater > release_base);
    assert!(major_greater > release_less);
    assert!(major_greater > release_greater);

    assert!(minor_base < release_base);
    assert!(minor_base < release_less);
    assert!(minor_base < release_greater);

    assert!(minor_greater > release_base);
    assert!(minor_greater > release_less);
    assert!(minor_greater > release_greater);

    assert!(minor_less < release_base);
    assert!(minor_less < release_less);
    assert!(minor_less < release_greater);
}
