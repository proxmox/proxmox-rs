use std::fmt::Display;

use serde::{Deserialize, Serialize};

use proxmox_config_digest::ConfigDigest;
use proxmox_schema::api;

#[api]
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
/// The type of format used for an APT repository.
pub enum APTRepositoryFileType {
    /// One-line-style format
    List,
    /// DEB822-style format
    Sources,
}

serde_plain::derive_display_from_serialize!(APTRepositoryFileType);
serde_plain::derive_fromstr_from_deserialize!(APTRepositoryFileType);

#[api]
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
/// The type of an APT package.
pub enum APTRepositoryPackageType {
    /// Debian package
    Deb,
    /// Debian source package
    DebSrc,
}

serde_plain::derive_display_from_serialize!(APTRepositoryPackageType);
serde_plain::derive_fromstr_from_deserialize!(APTRepositoryPackageType);

#[api(
    properties: {
        Key: {
            description: "Option key.",
            type: String,
        },
        Values: {
            description: "Option values.",
            type: Array,
            items: {
                description: "Value.",
                type: String,
            },
        },
    },
)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")] // for consistency
/// Additional options for an APT repository.
/// Used for both single- and multi-value options.
pub struct APTRepositoryOption {
    /// Option key.
    pub key: String,
    /// Option value(s).
    pub values: Vec<String>,
}

#[api(
    properties: {
        Types: {
            description: "List of package types.",
            type: Array,
            items: {
                type: APTRepositoryPackageType,
            },
        },
        URIs: {
            description: "List of repository URIs.",
            type: Array,
            items: {
                description: "Repository URI.",
                type: String,
            },
        },
        Suites: {
            description: "List of distributions.",
            type: Array,
            items: {
                description: "Package distribution.",
                type: String,
            },
        },
        Components: {
            description: "List of repository components.",
            type: Array,
            items: {
                description: "Repository component.",
                type: String,
            },
        },
        Options: {
            type: Array,
            optional: true,
            items: {
                type: APTRepositoryOption,
            },
        },
        Comment: {
            description: "Associated comment.",
            type: String,
            optional: true,
        },
        FileType: {
            type: APTRepositoryFileType,
        },
        Enabled: {
            description: "Whether the repository is enabled or not.",
            type: Boolean,
        },
    },
)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
/// Describes an APT repository.
pub struct APTRepository {
    /// List of package types.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<APTRepositoryPackageType>,

    /// List of repository URIs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[serde(rename = "URIs")]
    pub uris: Vec<String>,

    /// List of package distributions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suites: Vec<String>,

    /// List of repository components.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<String>,

    /// Additional options.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<APTRepositoryOption>,

    /// Associated comment.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment: String,

    /// Format of the defining file.
    pub file_type: APTRepositoryFileType,

    /// Whether the repository is enabled or not.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    pub enabled: bool,
}

#[api(
    properties: {
        "file-type": {
            type: APTRepositoryFileType,
        },
        repositories: {
            description: "List of APT repositories.",
            type: Array,
            items: {
                type: APTRepository,
            },
        },
        digest: {
            type: ConfigDigest,
            optional: true,
        },
    },
)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Represents an abstract APT repository file.
pub struct APTRepositoryFile {
    /// The path to the file. If None, `contents` must be set directly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// The type of the file.
    pub file_type: APTRepositoryFileType,

    /// List of repositories in the file.
    pub repositories: Vec<APTRepository>,

    /// The file content, if already parsed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Digest of the original contents.
    // We cannot use ConfigDigest here for compatibility reasons.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<[u8; 32]>,
}

#[api]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Error type for problems with APT repository files.
pub struct APTRepositoryFileError {
    /// The path to the problematic file.
    pub path: String,

    /// The error message.
    pub error: String,
}

impl Display for APTRepositoryFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "proxmox-apt error for '{}' - {}", self.path, self.error)
    }
}

impl std::error::Error for APTRepositoryFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

#[api]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Additional information for a repository.
pub struct APTRepositoryInfo {
    /// Path to the defining file.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,

    /// Index of the associated repository within the file (starting from 0).
    pub index: usize,

    /// The property from which the info originates (e.g. "Suites")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property: Option<String>,

    /// Info kind (e.g. "warning")
    pub kind: String,

    /// Info message
    pub message: String,
}

#[api(
    properties: {
        handle: {
            description: "Handle referencing a standard repository.",
            type: String,
        },
    },
)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
/// Reference to a standard repository and configuration status.
pub struct APTStandardRepository {
    /// Handle referencing a standard repository.
    pub handle: APTRepositoryHandle,

    /// Configuration status of the associated repository, where `None` means
    /// not configured, and `Some(bool)` indicates enabled or disabled.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "proxmox_serde::perl::deserialize_bool"
    )]
    pub status: Option<bool>,

    /// Display name of the repository.
    pub name: String,

    /// Description of the repository.
    pub description: String,
}

#[api]
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
/// Handles for Proxmox repositories.
pub enum APTRepositoryHandle {
    /// The enterprise repository for production use.
    Enterprise,
    /// The repository that can be used without subscription.
    NoSubscription,
    /// The test repository.
    Test,
    // TODO: Add separate enum for ceph releases and use something like
    // `CephTest(CephReleaseCodename),` once the API macro supports it.
    // Or create dedicated product type where ceph (or ceph$release) are entries.
    /// Ceph Squid enterprise repository.
    CephSquidEnterprise,
    /// Ceph Squid no-subscription repository.
    CephSquidNoSubscription,
    /// Ceph Squid test repository.
    CephSquidTest,
}

serde_plain::derive_display_from_serialize!(APTRepositoryHandle);
serde_plain::derive_fromstr_from_deserialize!(APTRepositoryHandle);

#[api()]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
/// Describes a package for which an update is available.
pub struct APTUpdateInfo {
    /// Package name
    pub package: String,
    /// Package title
    pub title: String,
    /// Package architecture
    pub arch: String,
    /// Human readable package description
    pub description: String,
    /// New version to be updated to
    pub version: String,
    /// Old version currently installed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_version: Option<String>,
    /// Package origin
    pub origin: String,
    /// Package priority in human-readable form
    pub priority: String,
    /// Package section
    pub section: String,
    /// Custom extra field for additional package information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_info: Option<String>,
}

#[api(
    properties: {
        notify: {
            default: false,
            optional: true,
        },
        quiet: {
            default: false,
            optional: true,
        },
    }
)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Options for APT update
pub struct APTUpdateOptions {
    /// Send notification mail about new package updates available to the email
    /// address configured for 'root@pam').
    pub notify: Option<bool>,
    /// Only produces output suitable for logging, omitting progress indicators.
    pub quiet: Option<bool>,
}

#[api(
    properties: {
        files: {
            type: Array,
            items: {
                type: APTRepositoryFile,
            },
        },
        errors: {
            type: Array,
            items: {
                type: APTRepositoryFileError,
            },
        },
        infos: {
            type: Array,
            items: {
                type: APTRepositoryInfo,
            },
        },
        "standard-repos": {
            type: Array,
            items: {
                type: APTStandardRepository,
            },
        },
        digest: {
            type: ConfigDigest,
        },
    },
)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Result from parsing the APT repository files in /etc/apt/.
pub struct APTRepositoriesResult {
    /// List of problematic files.
    pub errors: Vec<APTRepositoryFileError>,
    /// List of standard repositories and their configuration status.
    pub standard_repos: Vec<APTStandardRepository>,
    /// List of additional information/warnings about the repositories
    pub infos: Vec<APTRepositoryInfo>,
    /// List of parsed repository files.
    pub files: Vec<APTRepositoryFile>,
    pub digest: ConfigDigest,
}

#[api()]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Options for the get changelog API.
pub struct APTGetChangelogOptions {
    /// Package name to get changelog of.
    pub name: String,
    /// Package version to get changelog of. Omit to use candidate version.
    pub version: Option<String>,
}

#[api()]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Options for the change repository API call
pub struct APTChangeRepositoryOptions {
    /// Whether the repository should be enabled or not.
    pub enabled: Option<bool>,
}
