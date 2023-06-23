use std::collections::HashMap;

use anyhow::{bail, format_err, Error};
use rfc822_like::de::Deserializer;
use serde::Deserialize;
use serde_json::Value;

use super::CheckSums;
//Uploaders
//
//Homepage
//
//Version Control System (VCS) fields
//
//Testsuite
//
//Dgit
//
//Standards-Version (mandatory)
//
//Build-Depends et al
//
//Package-List (recommended)
//
//Checksums-Sha1 and Checksums-Sha256 (mandatory)
//
//Files (mandatory)

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SourcesFileRaw {
    pub format: String,
    pub package: String,
    pub binary: Option<Vec<String>>,
    pub version: String,
    pub section: Option<String>,
    pub priority: Option<String>,
    pub maintainer: String,
    pub uploaders: Option<String>,
    pub architecture: Option<String>,
    pub directory: String,
    pub files: String,
    #[serde(rename = "Checksums-Sha256")]
    pub sha256: Option<String>,
    #[serde(rename = "Checksums-Sha512")]
    pub sha512: Option<String>,
    #[serde(flatten)]
    pub extra_fields: HashMap<String, Value>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SourcePackageEntry {
    pub format: String,
    pub package: String,
    pub binary: Option<Vec<String>>,
    pub version: String,
    pub architecture: Option<String>,
    pub section: Option<String>,
    pub priority: Option<String>,
    pub maintainer: String,
    pub uploaders: Option<String>,
    pub directory: String,
    pub files: HashMap<String, SourcePackageFileReference>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SourcePackageFileReference {
    pub file: String,
    pub size: usize,
    pub checksums: CheckSums,
}

impl SourcePackageEntry {
    pub fn size(&self) -> usize {
        self.files.values().map(|f| f.size).sum()
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
/// A parsed representation of a Release file
pub struct SourcesFile {
    pub source_packages: Vec<SourcePackageEntry>,
}

impl TryFrom<SourcesFileRaw> for SourcePackageEntry {
    type Error = Error;

    fn try_from(value: SourcesFileRaw) -> Result<Self, Self::Error> {
        let mut parsed = SourcePackageEntry {
            package: value.package,
            binary: value.binary,
            version: value.version,
            architecture: value.architecture,
            files: HashMap::new(),
            format: value.format,
            section: value.section,
            priority: value.priority,
            maintainer: value.maintainer,
            uploaders: value.uploaders,
            directory: value.directory,
        };

        for file_reference in value.files.lines() {
            let (file_name, size, md5) = parse_file_reference(file_reference, 16)?;
            let entry = parsed.files.entry(file_name.clone()).or_insert_with(|| {
                SourcePackageFileReference {
                    file: file_name,
                    size,
                    checksums: CheckSums::default(),
                }
            });
            entry.checksums.md5 = Some(
                md5.try_into()
                    .map_err(|_| format_err!("unexpected checksum length"))?,
            );
            if entry.size != size {
                bail!("Size mismatch: {} != {}", entry.size, size);
            }
        }

        if let Some(sha256) = value.sha256 {
            for line in sha256.lines() {
                let (file_name, size, sha256) = parse_file_reference(line, 32)?;
                let entry = parsed.files.entry(file_name.clone()).or_insert_with(|| {
                    SourcePackageFileReference {
                        file: file_name,
                        size,
                        checksums: CheckSums::default(),
                    }
                });
                entry.checksums.sha256 = Some(
                    sha256
                        .try_into()
                        .map_err(|_| format_err!("unexpected checksum length"))?,
                );
                if entry.size != size {
                    bail!("Size mismatch: {} != {}", entry.size, size);
                }
            }
        };

        if let Some(sha512) = value.sha512 {
            for line in sha512.lines() {
                let (file_name, size, sha512) = parse_file_reference(line, 64)?;
                let entry = parsed.files.entry(file_name.clone()).or_insert_with(|| {
                    SourcePackageFileReference {
                        file: file_name,
                        size,
                        checksums: CheckSums::default(),
                    }
                });
                entry.checksums.sha512 = Some(
                    sha512
                        .try_into()
                        .map_err(|_| format_err!("unexpected checksum length"))?,
                );
                if entry.size != size {
                    bail!("Size mismatch: {} != {}", entry.size, size);
                }
            }
        };

        for (file_name, reference) in &parsed.files {
            if !reference.checksums.is_secure() {
                bail!("no strong checksum found for source entry '{}'", file_name);
            }
        }

        Ok(parsed)
    }
}

impl TryFrom<String> for SourcesFile {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.as_bytes().try_into()
    }
}

impl TryFrom<&[u8]> for SourcesFile {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let deserialized = <Vec<SourcesFileRaw>>::deserialize(Deserializer::new(value))?;
        deserialized.try_into()
    }
}

impl TryFrom<Vec<SourcesFileRaw>> for SourcesFile {
    type Error = Error;

    fn try_from(value: Vec<SourcesFileRaw>) -> Result<Self, Self::Error> {
        let mut source_packages = Vec::with_capacity(value.len());
        for entry in value {
            let entry: SourcePackageEntry = entry.try_into()?;
            source_packages.push(entry);
        }

        Ok(Self { source_packages })
    }
}

fn parse_file_reference(line: &str, csum_len: usize) -> Result<(String, usize, Vec<u8>), Error> {
    let mut split = line.split_ascii_whitespace();

    let checksum = split
        .next()
        .ok_or_else(|| format_err!("Missing 'checksum' field."))?;
    if checksum.len() > csum_len * 2 {
        bail!(
            "invalid checksum length: '{}', expected {} bytes",
            checksum,
            csum_len
        );
    }

    let checksum = hex::decode(checksum)?;

    let size = split
        .next()
        .ok_or_else(|| format_err!("Missing 'size' field."))?
        .parse::<usize>()?;

    let file = split
        .next()
        .ok_or_else(|| format_err!("Missing 'file name' field."))?
        .to_string();

    Ok((file, size, checksum))
}

#[test]
pub fn test_deb_packages_file() {
    // NOTE: test is over an excerpt from packages starting with 0-9, a, b & z using:
    // http://snapshot.debian.org/archive/debian/20221017T212657Z/dists/bullseye/main/source/Sources.xz
    let input = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/deb822/sources/deb.debian.org_debian_dists_bullseye_main_source_Sources"
    ));

    let deserialized =
        <Vec<SourcesFileRaw>>::deserialize(Deserializer::new(input.as_bytes())).unwrap();
    assert_eq!(deserialized.len(), 1558);

    let parsed: SourcesFile = deserialized.try_into().unwrap();

    assert_eq!(parsed.source_packages.len(), 1558);

    let found = parsed
        .source_packages
        .iter()
        .find(|source| source.package == "base-files")
        .expect("test file contains 'base-files' entry");
    assert_eq!(found.package, "base-files");
    assert_eq!(found.format, "3.0 (native)");
    assert_eq!(found.architecture.as_deref(), Some("any"));
    assert_eq!(found.directory, "pool/main/b/base-files");
    assert_eq!(found.section.as_deref(), Some("admin"));
    assert_eq!(found.version, "11.1+deb11u5");

    let binary_packages = found
        .binary
        .as_ref()
        .expect("base-files source package builds base-files binary package");
    assert_eq!(binary_packages.len(), 1);
    assert_eq!(binary_packages[0], "base-files");

    let references = &found.files;
    assert_eq!(references.len(), 2);

    let dsc_file = "base-files_11.1+deb11u5.dsc";
    let dsc = references
        .get(dsc_file)
        .expect("base-files source package contains 'dsc' reference");
    assert_eq!(dsc.file, dsc_file);
    assert_eq!(dsc.size, 1110);
    assert_eq!(
        dsc.checksums.md5.expect("dsc has md5 checksum"),
        hex::decode("741c34ac0151262a03de8d5a07bc4271").unwrap()[..]
    );
    assert_eq!(
        dsc.checksums.sha256.expect("dsc has sha256 checksum"),
        hex::decode("c41a7f00d57759f27e6068240d1ea7ad80a9a752e4fb43850f7e86e967422bd3").unwrap()[..]
    );

    let tar_file = "base-files_11.1+deb11u5.tar.xz";
    let tar = references
        .get(tar_file)
        .expect("base-files source package contains 'tar' reference");
    assert_eq!(tar.file, tar_file);
    assert_eq!(tar.size, 65612);
    assert_eq!(
        tar.checksums.md5.expect("tar has md5 checksum"),
        hex::decode("995df33642118b566a4026410e1c6aac").unwrap()[..]
    );
    assert_eq!(
        tar.checksums.sha256.expect("tar has sha256 checksum"),
        hex::decode("31c9e5745845a73f3d5c8a7868c379d77aaca42b81194679d7ab40cc28e3a0e9").unwrap()[..]
    );
}
