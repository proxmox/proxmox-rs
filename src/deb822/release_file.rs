use std::collections::HashMap;

use anyhow::{bail, format_err, Error};
use rfc822_like::de::Deserializer;
use serde::Deserialize;
use serde_json::Value;

use super::CheckSums;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ReleaseFileRaw {
    pub architectures: Option<String>,
    pub changelogs: Option<String>,
    pub codename: Option<String>,
    pub components: Option<String>,
    pub date: Option<String>,
    pub description: Option<String>,
    pub label: Option<String>,
    pub origin: Option<String>,
    pub suite: Option<String>,
    pub version: Option<String>,

    #[serde(rename = "MD5Sum")]
    pub md5_sum: Option<String>,
    #[serde(rename = "SHA1")]
    pub sha1: Option<String>,
    #[serde(rename = "SHA256")]
    pub sha256: Option<String>,
    #[serde(rename = "SHA512")]
    pub sha512: Option<String>,

    #[serde(flatten)]
    pub extra_fields: HashMap<String, Value>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompressionType {
    Bzip2,
    Gzip,
    Lzma,
    Xz,
}

pub type Architecture = String;
pub type Component = String;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
/// Type of file reference extraced from path.
///
/// `Packages` and `Sources` will contain further reference to binary or source package files.
///  These are handled in `PackagesFile` and `SourcesFile` respectively.
pub enum FileReferenceType {
    /// A `Contents` index listing contents of binary packages
    Contents(Architecture, Option<CompressionType>),
    /// A `Contents` index listing contents of binary udeb packages
    ContentsUdeb(Architecture, Option<CompressionType>),
    /// A DEP11 `Components` metadata file or `icons` archive
    Dep11(Option<CompressionType>),
    /// Referenced files which are not really part of the APT repository but only signed for trust-anchor reasons
    Ignored,
    /// PDiff indices
    PDiff,
    /// A `Packages` index listing binary package metadata and references
    Packages(Architecture, Option<CompressionType>),
    /// A compat `Release` file with no relevant content
    PseudoRelease(Option<Architecture>),
    /// A `Sources` index listing source package metadata and references
    Sources(Option<CompressionType>),
    /// A `Translation` file
    Translation(Option<CompressionType>),
    /// Unknown file reference
    Unknown,
}

impl FileReferenceType {
    fn match_compression(value: &str) -> Result<Option<CompressionType>, Error> {
        if value.is_empty() {
            return Ok(None);
        }

        let value = if let Some(stripped) = value.strip_prefix('.') {
            stripped
        } else {
            value
        };

        match value {
            "bz2" => Ok(Some(CompressionType::Bzip2)),
            "gz" => Ok(Some(CompressionType::Gzip)),
            "lzma" => Ok(Some(CompressionType::Lzma)),
            "xz" => Ok(Some(CompressionType::Xz)),
            other => bail!("Unexpected file extension '{other}'."),
        }
    }
    pub fn parse(component: &str, path: &str) -> Result<FileReferenceType, Error> {
        // everything referenced in a release file should be component-specific
        let rest = path
            .strip_prefix(&format!("{component}/"))
            .ok_or_else(|| format_err!("Doesn't start with component '{component}'"))?;

        let parse_binary_dir = |file_name: &str, arch: &str| {
            if let Some((dir, _rest)) = file_name.split_once('/') {
                if dir == "Packages.diff" {
                    // TODO re-evaluate?
                    Ok(FileReferenceType::PDiff)
                } else {
                    Ok(FileReferenceType::Unknown)
                }
            } else if file_name == "Release" {
                Ok(FileReferenceType::PseudoRelease(Some(arch.to_owned())))
            } else {
                let comp = match file_name.strip_prefix("Packages") {
                    None => {
                        bail!("found unexpected non-Packages reference to '{path}'")
                    }
                    Some(ext) => FileReferenceType::match_compression(ext)?,
                };
                //println!("compression: {comp:?}");
                Ok(FileReferenceType::Packages(arch.to_owned(), comp))
            }
        };

        if let Some((dir, rest)) = rest.split_once('/') {
            // reference into another subdir
            match dir {
                "source" => {
                    // Sources or compat-Release
                    if let Some((dir, _rest)) = rest.split_once('/') {
                        if dir == "Sources.diff" {
                            Ok(FileReferenceType::PDiff)
                        } else {
                            Ok(FileReferenceType::Unknown)
                        }
                    } else if rest == "Release" {
                        Ok(FileReferenceType::PseudoRelease(None))
                    } else if let Some(ext) = rest.strip_prefix("Sources") {
                        let comp = FileReferenceType::match_compression(ext)?;
                        Ok(FileReferenceType::Sources(comp))
                    } else {
                        Ok(FileReferenceType::Unknown)
                    }
                }
                "dep11" => {
                    if let Some((_path, ext)) = rest.rsplit_once('.') {
                        Ok(FileReferenceType::Dep11(
                            FileReferenceType::match_compression(ext).ok().flatten(),
                        ))
                    } else {
                        Ok(FileReferenceType::Dep11(None))
                    }
                }
                "debian-installer" => {
                    // another layer, then like regular repo but pointing at udebs
                    if let Some((dir, rest)) = rest.split_once('/') {
                        if let Some(arch) = dir.strip_prefix("binary-") {
                            // Packages or compat-Release
                            return parse_binary_dir(rest, arch);
                        }
                    }

                    // all the rest
                    Ok(FileReferenceType::Unknown)
                }
                "i18n" => {
                    if let Some((dir, _rest)) = rest.split_once('/') {
                        if dir.starts_with("Translation") && dir.ends_with(".diff") {
                            Ok(FileReferenceType::PDiff)
                        } else {
                            Ok(FileReferenceType::Unknown)
                        }
                    } else if let Some((_, ext)) = rest.split_once('.') {
                        Ok(FileReferenceType::Translation(
                            FileReferenceType::match_compression(ext)?,
                        ))
                    } else {
                        Ok(FileReferenceType::Translation(None))
                    }
                }
                _ => {
                    if let Some(arch) = dir.strip_prefix("binary-") {
                        // Packages or compat-Release
                        parse_binary_dir(rest, arch)
                    } else if let Some(_arch) = dir.strip_prefix("installer-") {
                        // netboot installer checksum files
                        Ok(FileReferenceType::Ignored)
                    } else {
                        // all the rest
                        Ok(FileReferenceType::Unknown)
                    }
                }
            }
        } else if let Some(rest) = rest.strip_prefix("Contents-") {
            // reference to a top-level file - Contents-*
            let (rest, udeb) = if let Some(rest) = rest.strip_prefix("udeb-") {
                (rest, true)
            } else {
                (rest, false)
            };
            let (arch, comp) = match rest.split_once('.') {
                Some((arch, comp_str)) => (
                    arch.to_owned(),
                    FileReferenceType::match_compression(comp_str)?,
                ),
                None => (rest.to_owned(), None),
            };
            if udeb {
                Ok(FileReferenceType::ContentsUdeb(arch, comp))
            } else {
                Ok(FileReferenceType::Contents(arch, comp))
            }
        } else {
            Ok(FileReferenceType::Unknown)
        }
    }

    pub fn compression(&self) -> Option<CompressionType> {
        match *self {
            FileReferenceType::Contents(_, comp)
            | FileReferenceType::ContentsUdeb(_, comp)
            | FileReferenceType::Packages(_, comp)
            | FileReferenceType::Sources(comp)
            | FileReferenceType::Translation(comp)
            | FileReferenceType::Dep11(comp) => comp,
            FileReferenceType::Unknown
            | FileReferenceType::PDiff
            | FileReferenceType::PseudoRelease(_)
            | FileReferenceType::Ignored => None,
        }
    }

    pub fn is_package_index(&self) -> bool {
        matches!(self, FileReferenceType::Packages(_, _))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileReference {
    pub path: String,
    pub size: usize,
    pub checksums: CheckSums,
    pub component: Component,
    pub file_type: FileReferenceType,
}

impl FileReference {
    pub fn basename(&self) -> Result<String, Error> {
        match self.file_type.compression() {
            Some(_) => {
                let (base, _ext) = self
                    .path
                    .rsplit_once('.')
                    .ok_or_else(|| format_err!("compressed file without file extension"))?;
                Ok(base.to_owned())
            }
            None => Ok(self.path.clone()),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
/// A parsed representation of a Release file
pub struct ReleaseFile {
    /// List of architectures, e.g., `amd64` or `all`.
    pub architectures: Vec<String>,
    // TODO No-Support-for-Architecture-all
    /// URL for changelog queries via `apt changelog`.
    pub changelogs: Option<String>,
    /// Release codename - single word, e.g., `bullseye`.
    pub codename: Option<String>,
    /// List of repository areas, e.g., `main`.
    pub components: Vec<String>,
    /// UTC timestamp of release file generation
    pub date: Option<u64>,
    /// UTC timestamp of release file expiration
    pub valid_until: Option<u64>,
    /// Repository description -
    // TODO exact format?
    pub description: Option<String>,
    /// Repository label - single line
    pub label: Option<String>,
    /// Repository origin - single line
    pub origin: Option<String>,
    /// Release suite - single word, e.g., `stable`.
    pub suite: Option<String>,
    /// Release version
    pub version: Option<String>,

    /// Whether by-hash retrieval of referenced files is possible
    pub aquire_by_hash: bool,

    /// Files referenced by this `Release` file, e.g., packages indices.
    ///
    /// Grouped by basename, since only the compressed version needs to actually exist on the repository server.
    pub files: HashMap<String, Vec<FileReference>>,
}

impl TryFrom<ReleaseFileRaw> for ReleaseFile {
    type Error = Error;

    fn try_from(value: ReleaseFileRaw) -> Result<Self, Self::Error> {
        let mut parsed = ReleaseFile::default();

        let parse_whitespace_list = |list_str: String| {
            list_str
                .split_ascii_whitespace()
                .map(|arch| arch.to_owned())
                .collect::<Vec<String>>()
        };

        let parse_date = |_date_str: String| {
            // TODO implement
            0
        };

        parsed.architectures = parse_whitespace_list(
            value
                .architectures
                .ok_or_else(|| format_err!("'Architectures' field missing."))?,
        );
        parsed.components = parse_whitespace_list(
            value
                .components
                .ok_or_else(|| format_err!("'Components' field missing."))?,
        );

        parsed.changelogs = value.changelogs;
        parsed.codename = value.codename;

        parsed.date = value.date.map(parse_date);
        parsed.valid_until = value
            .extra_fields
            .get("Valid-Until")
            .map(|val| parse_date(val.to_string()));

        parsed.description = value.description;
        parsed.label = value.label;
        parsed.origin = value.origin;
        parsed.suite = value.suite;
        parsed.version = value.version;

        parsed.aquire_by_hash = match value.extra_fields.get("Aquire-By-Hash") {
            Some(val) => *val == "yes",
            None => false,
        };

        // Fixup bullseye-security release files which have invalid components
        if parsed.label.as_deref() == Some("Debian-Security")
            && parsed.codename.as_deref() == Some("bullseye-security")
        {
            parsed.components = parsed
                .components
                .into_iter()
                .map(|comp| {
                    if let Some(stripped) = comp.strip_prefix("updates/") {
                        stripped.to_owned()
                    } else {
                        comp
                    }
                })
                .collect();
        }

        let mut references_map: HashMap<String, HashMap<String, FileReference>> = HashMap::new();

        let parse_file_reference = |line: &str, csum_len: usize, components: &[String]| {
            let mut split = line.split_ascii_whitespace();
            let checksum = split.next().ok_or_else(|| format_err!("bla"))?;
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
                .ok_or_else(|| format_err!("No 'size' field in file reference line."))?
                .parse::<usize>()?;

            let file = split
                .next()
                .ok_or_else(|| format_err!("No 'path' field in file reference line."))?
                .to_string();

            let (component, file_type) = components
                .iter()
                .find_map(|component| {
                    FileReferenceType::parse(component, &file)
                        .ok()
                        .map(|file_type| (component.clone(), file_type))
                })
                .ok_or_else(|| format_err!("failed to parse file reference '{file}'"))?;

            Ok((
                FileReference {
                    path: file,
                    size,
                    checksums: CheckSums::default(),
                    component,
                    file_type,
                },
                checksum,
            ))
        };

        let merge_references = |base_map: &mut HashMap<String, HashMap<String, FileReference>>,
                                file_ref: FileReference| {
            let base = file_ref.basename()?;

            match base_map.get_mut(&base) {
                None => {
                    let mut map = HashMap::new();
                    map.insert(file_ref.path.clone(), file_ref);
                    base_map.insert(base, map);
                }
                Some(entries) => {
                    match entries.get_mut(&file_ref.path) {
                        Some(entry) => {
                            if entry.size != file_ref.size {
                                bail!(
                                    "Multiple entries for '{}' with size mismatch: {} / {}",
                                    entry.path,
                                    file_ref.size,
                                    entry.size
                                );
                            }

                            entry.checksums.merge(&file_ref.checksums).map_err(|err| {
                                format_err!("Multiple checksums for '{}' - {err}", entry.path)
                            })?;
                        }
                        None => {
                            entries.insert(file_ref.path.clone(), file_ref);
                        }
                    };
                }
            };

            Ok(())
        };

        if let Some(md5) = value.md5_sum {
            for line in md5.lines() {
                let (mut file_ref, checksum) =
                    parse_file_reference(line, 16, parsed.components.as_ref())?;

                let checksum = checksum
                    .try_into()
                    .map_err(|_err| format_err!("unexpected checksum length"))?;

                file_ref.checksums.md5 = Some(checksum);

                merge_references(&mut references_map, file_ref)?;
            }
        }

        if let Some(sha1) = value.sha1 {
            for line in sha1.lines() {
                let (mut file_ref, checksum) =
                    parse_file_reference(line, 20, parsed.components.as_ref())?;
                let checksum = checksum
                    .try_into()
                    .map_err(|_err| format_err!("unexpected checksum length"))?;

                file_ref.checksums.sha1 = Some(checksum);
                merge_references(&mut references_map, file_ref)?;
            }
        }

        if let Some(sha256) = value.sha256 {
            for line in sha256.lines() {
                let (mut file_ref, checksum) =
                    parse_file_reference(line, 32, parsed.components.as_ref())?;
                let checksum = checksum
                    .try_into()
                    .map_err(|_err| format_err!("unexpected checksum length"))?;

                file_ref.checksums.sha256 = Some(checksum);
                merge_references(&mut references_map, file_ref)?;
            }
        }

        if let Some(sha512) = value.sha512 {
            for line in sha512.lines() {
                let (mut file_ref, checksum) =
                    parse_file_reference(line, 64, parsed.components.as_ref())?;
                let checksum = checksum
                    .try_into()
                    .map_err(|_err| format_err!("unexpected checksum length"))?;

                file_ref.checksums.sha512 = Some(checksum);
                merge_references(&mut references_map, file_ref)?;
            }
        }

        parsed.files =
            references_map
                .into_iter()
                .fold(HashMap::new(), |mut map, (base, inner_map)| {
                    map.insert(base, inner_map.into_values().collect());
                    map
                });

        if let Some(insecure) = parsed
            .files
            .values()
            .flatten()
            .find(|file| !file.checksums.is_secure())
        {
            bail!(
                "found file reference without strong checksum: {}",
                insecure.path
            );
        }

        Ok(parsed)
    }
}

impl TryFrom<String> for ReleaseFile {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.as_bytes().try_into()
    }
}

impl TryFrom<&[u8]> for ReleaseFile {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let deserialized = ReleaseFileRaw::deserialize(Deserializer::new(value))?;
        deserialized.try_into()
    }
}

#[test]
pub fn test_deb_release_file() {
    let input = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/deb822/release/deb.debian.org_debian_dists_bullseye_Release"
    ));

    let deserialized = ReleaseFileRaw::deserialize(Deserializer::new(input.as_bytes())).unwrap();
    //println!("{:?}", deserialized);

    let parsed: ReleaseFile = deserialized.try_into().unwrap();
    //println!("{:?}", parsed);

    assert_eq!(parsed.files.len(), 315);
}

#[test]
pub fn test_deb_release_file_insecure() {
    let input = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/deb822/release/deb.debian.org_debian_dists_bullseye_Release_insecure"
    ));

    let deserialized = ReleaseFileRaw::deserialize(Deserializer::new(input.as_bytes())).unwrap();
    //println!("{:?}", deserialized);

    let parsed: Result<ReleaseFile, Error> = deserialized.try_into();
    assert!(parsed.is_err());

    println!("{:?}", parsed);
}
