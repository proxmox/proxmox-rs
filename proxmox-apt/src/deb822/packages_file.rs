use std::collections::HashMap;

use anyhow::{bail, Error};
use rfc822_like::de::Deserializer;
use serde::Deserialize;
use serde_json::Value;

use super::CheckSums;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PackagesFileRaw {
    pub package: String,
    pub source: Option<String>,
    pub version: String,
    pub section: Option<String>,
    pub priority: String,
    pub architecture: String,
    pub essential: Option<String>,
    pub depends: Option<String>,
    pub recommends: Option<String>,
    pub suggests: Option<String>,
    pub breaks: Option<String>,
    pub conflicts: Option<String>,
    #[serde(rename = "Installed-Size")]
    pub installed_size: Option<String>,
    pub maintainer: String,
    pub description: String,
    pub filename: String,
    pub size: String,
    #[serde(rename = "Multi-Arch")]
    pub multi_arch: Option<String>,

    #[serde(rename = "MD5sum")]
    pub md5_sum: Option<String>,
    #[serde(rename = "SHA1")]
    pub sha1: Option<String>,
    #[serde(rename = "SHA256")]
    pub sha256: Option<String>,
    #[serde(rename = "SHA512")]
    pub sha512: Option<String>,

    #[serde(rename = "Description-md5")]
    pub description_md5: Option<String>,

    #[serde(flatten)]
    pub extra_fields: HashMap<String, Value>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PackageEntry {
    pub package: String,
    pub source: Option<String>,
    pub version: String,
    pub architecture: String,
    pub file: String,
    pub size: usize,
    pub installed_size: Option<usize>,
    pub checksums: CheckSums,
    pub section: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
/// A parsed representation of a Release file
pub struct PackagesFile {
    pub files: Vec<PackageEntry>,
}

impl TryFrom<PackagesFileRaw> for PackageEntry {
    type Error = Error;

    fn try_from(value: PackagesFileRaw) -> Result<Self, Self::Error> {
        let installed_size = match value.installed_size {
            Some(val) => Some(val.parse::<usize>()?),
            None => None,
        };

        let mut parsed = PackageEntry {
            package: value.package,
            source: value.source,
            version: value.version,
            architecture: value.architecture,
            file: value.filename,
            size: value.size.parse::<usize>()?,
            installed_size,
            checksums: CheckSums::default(),
            section: value.section.unwrap_or("unknown".to_owned()),
        };

        if let Some(md5) = value.md5_sum {
            let mut bytes = [0u8; 16];
            hex::decode_to_slice(md5, &mut bytes)?;
            parsed.checksums.md5 = Some(bytes);
        };

        if let Some(sha1) = value.sha1 {
            let mut bytes = [0u8; 20];
            hex::decode_to_slice(sha1, &mut bytes)?;
            parsed.checksums.sha1 = Some(bytes);
        };

        if let Some(sha256) = value.sha256 {
            let mut bytes = [0u8; 32];
            hex::decode_to_slice(sha256, &mut bytes)?;
            parsed.checksums.sha256 = Some(bytes);
        };

        if let Some(sha512) = value.sha512 {
            let mut bytes = [0u8; 64];
            hex::decode_to_slice(sha512, &mut bytes)?;
            parsed.checksums.sha512 = Some(bytes);
        };

        if !parsed.checksums.is_secure() {
            bail!(
                "no strong checksum found for package entry '{}'",
                parsed.package
            );
        }

        Ok(parsed)
    }
}

impl TryFrom<String> for PackagesFile {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.as_bytes().try_into()
    }
}

impl TryFrom<&[u8]> for PackagesFile {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let deserialized = <Vec<PackagesFileRaw>>::deserialize(Deserializer::new(value))?;
        deserialized.try_into()
    }
}

impl TryFrom<Vec<PackagesFileRaw>> for PackagesFile {
    type Error = Error;

    fn try_from(value: Vec<PackagesFileRaw>) -> Result<Self, Self::Error> {
        let mut files = Vec::with_capacity(value.len());
        for entry in value {
            let entry: PackageEntry = entry.try_into()?;
            files.push(entry);
        }

        Ok(Self { files })
    }
}

#[test]
pub fn test_deb_packages_file() {
    let input = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/deb822/packages/deb.debian.org_debian_dists_bullseye_main_binary-amd64_Packages"
    ));

    let deserialized =
        <Vec<PackagesFileRaw>>::deserialize(Deserializer::new(input.as_bytes())).unwrap();
    //println!("{:?}", deserialized);

    let parsed: PackagesFile = deserialized.try_into().unwrap();
    //println!("{:?}", parsed);

    assert_eq!(parsed.files.len(), 58618);
}
