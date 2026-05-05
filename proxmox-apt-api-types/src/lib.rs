use std::fmt::Display;

use serde::{Deserialize, Serialize};

use proxmox_config_digest::ConfigDigest;
use proxmox_schema::{api, const_regex, ApiStringFormat};

const_regex! {
    pub PACKAGE_NAME_REGEX = r"^[a-z0-9][-+.a-z0-9:]+$";
    /// Valid `Unknown(_)` payload shape for the open-enum APT wire types.
    pub APT_OPEN_ENUM_REGEX = r"^[a-z][a-z0-9]*(-[a-z0-9]+)*$";
}
const PACKAGE_NAME_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&PACKAGE_NAME_REGEX);

fn is_apt_open_enum_token(s: &str) -> bool {
    APT_OPEN_ENUM_REGEX.is_match(s)
}

/// API-types-level error for APT repository data; avoids pulling in `anyhow`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct APTRepositoryError {
    error: String,
}

impl APTRepositoryError {
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
        }
    }
}

impl Display for APTRepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "proxmox-apt error - {}", self.error)
    }
}

impl std::error::Error for APTRepositoryError {}

#[api]
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
/// Debian release codenames, excluding `sid`. `Unknown` sorts above all known so future
/// codenames take the newer branch in `*suite >= Trixie`-style checks. Wire-deserialize
/// accepts unknown values; `TryFrom<&str>` rejects them.
pub enum DebianCodename {
    /// Debian 5 Lenny
    Lenny,
    /// Debian 6 Squeeze
    Squeeze,
    /// Debian 7 Wheezy
    Wheezy,
    /// Debian 8 Jessie
    Jessie,
    /// Debian 9 Stretch
    Stretch,
    /// Debian 10 Buster
    Buster,
    /// Debian 11 Bullseye
    Bullseye,
    /// Debian 12 Bookworm
    Bookworm,
    /// Debian 13 Trixie
    Trixie,
    /// Debian 14 Forky
    Forky,
    /// Debian 15 Duke
    Duke,
    /// Forward-compat fallback for unknown wire values; stored lowercased.
    #[serde(untagged)]
    Unknown(String),
}

impl DebianCodename {
    /// Stable ordinal driving [`Ord`]; `Unknown` is `u8::MAX` so unknown codenames sort highest.
    fn rank(&self) -> u8 {
        match self {
            Self::Lenny => 1,
            Self::Squeeze => 2,
            Self::Wheezy => 3,
            Self::Jessie => 4,
            Self::Stretch => 5,
            Self::Buster => 6,
            Self::Bullseye => 7,
            Self::Bookworm => 8,
            Self::Trixie => 9,
            Self::Forky => 10,
            Self::Duke => 11,
            Self::Unknown(_) => u8::MAX,
        }
    }
}

impl Ord for DebianCodename {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Self::Unknown(a), Self::Unknown(b)) => a.cmp(b),
            _ => self.rank().cmp(&other.rank()),
        }
    }
}

impl PartialOrd for DebianCodename {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

proxmox_serde::forward_display_to_serialize!(DebianCodename);
proxmox_serde::forward_from_str_to_deserialize!(DebianCodename);

impl DebianCodename {
    /// Lowercase wire-string lookup shared by `Deserialize` and `TryFrom<&str>`.
    fn from_known(s: &str) -> Option<Self> {
        Some(match s {
            "lenny" => Self::Lenny,
            "squeeze" => Self::Squeeze,
            "wheezy" => Self::Wheezy,
            "jessie" => Self::Jessie,
            "stretch" => Self::Stretch,
            "buster" => Self::Buster,
            "bullseye" => Self::Bullseye,
            "bookworm" => Self::Bookworm,
            "trixie" => Self::Trixie,
            "forky" => Self::Forky,
            "duke" => Self::Duke,
            _ => return None,
        })
    }

    /// Next known codename, or `None` for the latest known release or any `Unknown`.
    pub fn next(&self) -> Option<Self> {
        Some(match self {
            Self::Lenny => Self::Squeeze,
            Self::Squeeze => Self::Wheezy,
            Self::Wheezy => Self::Jessie,
            Self::Jessie => Self::Stretch,
            Self::Stretch => Self::Buster,
            Self::Buster => Self::Bullseye,
            Self::Bullseye => Self::Bookworm,
            Self::Bookworm => Self::Trixie,
            Self::Trixie => Self::Forky,
            Self::Forky => Self::Duke,
            Self::Duke | Self::Unknown(_) => return None,
        })
    }
}

impl<'de> Deserialize<'de> for DebianCodename {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let lower = String::deserialize(d)?.to_ascii_lowercase();
        if let Some(known) = Self::from_known(&lower) {
            return Ok(known);
        }
        if !is_apt_open_enum_token(&lower) {
            return Err(serde::de::Error::custom(format!(
                "invalid Debian codename {lower:?}"
            )));
        }
        Ok(Self::Unknown(lower))
    }
}

impl TryFrom<&str> for DebianCodename {
    type Error = APTRepositoryError;

    fn try_from(string: &str) -> Result<Self, Self::Error> {
        Self::from_known(&string.to_ascii_lowercase())
            .ok_or_else(|| APTRepositoryError::new(format!("unknown Debian code name '{string}'")))
    }
}

/// Proxmox host product an APT operation runs on; scopes offered repositories and file layout.
#[api]
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum HostProduct {
    /// Proxmox VE
    Pve,
    /// Proxmox Backup Server
    Pbs,
    /// Proxmox Datacenter Manager
    Pdm,
    /// Proxmox Mail Gateway
    Pmg,
    /// Forward-compat fallback for unknown wire values; stored lowercased.
    #[serde(untagged)]
    Unknown(String),
}

proxmox_serde::forward_display_to_serialize!(HostProduct);
proxmox_serde::forward_from_str_to_deserialize!(HostProduct);

impl<'de> Deserialize<'de> for HostProduct {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let lower = String::deserialize(d)?.to_ascii_lowercase();
        Ok(match lower.as_str() {
            "pve" => Self::Pve,
            "pbs" => Self::Pbs,
            "pdm" => Self::Pdm,
            "pmg" => Self::Pmg,
            _ if is_apt_open_enum_token(&lower) => Self::Unknown(lower),
            _ => {
                return Err(serde::de::Error::custom(format!(
                    "invalid host product {lower:?}"
                )));
            }
        })
    }
}

impl HostProduct {
    /// Canonical kebab-case identifier used in URIs, paths, and repository components.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Pve => "pve",
            Self::Pbs => "pbs",
            Self::Pdm => "pdm",
            Self::Pmg => "pmg",
            Self::Unknown(s) => s.as_str(),
        }
    }
}

/// Software a repository provides; with an [`APTRepoComponent`] and [`DebianCodename`]
/// it identifies a standard repository.
#[api]
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum APTRepoType {
    /// Proxmox VE
    Pve,
    /// Proxmox Backup Server
    Pbs,
    /// Proxmox Backup Server Client-only repository
    PbsClient,
    /// Proxmox Datacenter Manager
    Pdm,
    /// Proxmox Mail Gateway
    Pmg,
    /// Debian main archive
    Debian,
    /// Debian security archive
    DebianSecurity,
    /// Debian backports archive
    DebianBackports,
    /// Proxmox Ceph Squid
    CephSquid,
    /// Proxmox Ceph Tentacle
    CephTentacle,
    /// Forward-compat fallback for unknown wire values; stored lowercased.
    #[serde(untagged)]
    Unknown(String),
}

proxmox_serde::forward_display_to_serialize!(APTRepoType);
proxmox_serde::forward_from_str_to_deserialize!(APTRepoType);

impl<'de> Deserialize<'de> for APTRepoType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let lower = String::deserialize(d)?.to_ascii_lowercase();
        Ok(match lower.as_str() {
            "pve" => Self::Pve,
            "pbs" => Self::Pbs,
            "pbs-client" => Self::PbsClient,
            "pdm" => Self::Pdm,
            "pmg" => Self::Pmg,
            "debian" => Self::Debian,
            "debian-security" => Self::DebianSecurity,
            "debian-backports" => Self::DebianBackports,
            "ceph-squid" => Self::CephSquid,
            "ceph-tentacle" => Self::CephTentacle,
            _ if is_apt_open_enum_token(&lower) => Self::Unknown(lower),
            _ => {
                return Err(serde::de::Error::custom(format!(
                    "invalid APT repo type {lower:?}"
                )));
            }
        })
    }
}

impl APTRepoType {
    /// Whether this repo type is a Proxmox host product; exhaustive so new variants fail to compile.
    pub fn is_host_product(&self) -> bool {
        match self {
            Self::Pve | Self::Pbs | Self::Pdm | Self::Pmg => true,
            Self::PbsClient
            | Self::Debian
            | Self::DebianSecurity
            | Self::DebianBackports
            | Self::CephSquid
            | Self::CephTentacle
            | Self::Unknown(_) => false,
        }
    }

    /// Whether this repo type ships Ceph release packages; exhaustive (new variants fail to compile).
    pub fn is_ceph_release(&self) -> bool {
        match self {
            Self::CephSquid | Self::CephTentacle => true,
            Self::Pve
            | Self::Pbs
            | Self::PbsClient
            | Self::Pdm
            | Self::Pmg
            | Self::Debian
            | Self::DebianSecurity
            | Self::DebianBackports
            | Self::Unknown(_) => false,
        }
    }
}

/// Release channel of a Proxmox repository; combined with [`APTRepoType`] identifies a repo.
#[api]
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum APTRepoComponent {
    /// The enterprise repository for production use.
    Enterprise,
    /// The repository that can be used without subscription.
    NoSubscription,
    /// The test repository.
    Test,
    /// Production repository for components without a no-subscription tier.
    Main,
    /// Internal staging repository for builds undergoing initial QA.
    Staging,
    /// Forward-compat fallback for unknown wire values; stored lowercased.
    #[serde(untagged)]
    Unknown(String),
}

proxmox_serde::forward_display_to_serialize!(APTRepoComponent);
proxmox_serde::forward_from_str_to_deserialize!(APTRepoComponent);

impl<'de> Deserialize<'de> for APTRepoComponent {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let lower = String::deserialize(d)?.to_ascii_lowercase();
        Ok(match lower.as_str() {
            "enterprise" => Self::Enterprise,
            "no-subscription" => Self::NoSubscription,
            "test" => Self::Test,
            "main" => Self::Main,
            "staging" => Self::Staging,
            _ if is_apt_open_enum_token(&lower) => Self::Unknown(lower),
            _ => {
                return Err(serde::de::Error::custom(format!(
                    "invalid APT repo component {lower:?}"
                )));
            }
        })
    }
}

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

proxmox_serde::forward_display_to_serialize!(APTRepositoryFileType);
proxmox_serde::forward_from_str_to_deserialize!(APTRepositoryFileType);

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

proxmox_serde::forward_display_to_serialize!(APTRepositoryPackageType);
proxmox_serde::forward_from_str_to_deserialize!(APTRepositoryPackageType);

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

/// Which host-product and Ceph-release channels are enabled across a set of
/// [`APTStandardRepository`] entries. Built via [`Self::from_repos`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct APTStandardRepoSummary {
    pub has_enterprise: bool,
    pub has_no_subscription: bool,
    pub has_test: bool,
    pub has_ceph_enterprise: bool,
    pub has_ceph_no_subscription: bool,
    pub has_ceph_test: bool,
    /// Enabled handles that didn't fit any known bucket; suitable for logging.
    pub unrecognized: Vec<APTRepositoryHandle>,
}

impl APTStandardRepoSummary {
    /// Walks the iterator once; only `status == Some(true)` entries contribute.
    pub fn from_repos<'a, I>(repos: I) -> Self
    where
        I: IntoIterator<Item = &'a APTStandardRepository>,
    {
        let mut s = Self::default();
        for r in repos {
            if r.status != Some(true) {
                continue;
            }
            let handle = &r.handle;
            let unknown_component = matches!(handle.component(), APTRepoComponent::Unknown(_));
            let is_ceph = match handle.repo_type() {
                None => false,
                Some(APTRepoType::Unknown(_)) => {
                    s.unrecognized.push(handle.clone());
                    continue;
                }
                Some(rt) => rt.is_ceph_release(),
            };
            if unknown_component {
                s.unrecognized.push(handle.clone());
                continue;
            }
            match (handle.repo_type().is_none(), is_ceph, handle.component()) {
                (true, _, APTRepoComponent::Enterprise) => s.has_enterprise = true,
                (true, _, APTRepoComponent::NoSubscription) => s.has_no_subscription = true,
                (true, _, APTRepoComponent::Test) => s.has_test = true,
                (false, true, APTRepoComponent::Enterprise) => s.has_ceph_enterprise = true,
                (false, true, APTRepoComponent::NoSubscription) => {
                    s.has_ceph_no_subscription = true
                }
                (false, true, APTRepoComponent::Test) => s.has_ceph_test = true,
                _ => {}
            }
        }
        s
    }
}

const_regex! {
    pub APT_REPOSITORY_HANDLE_REGEX = r"^[a-z][a-z0-9]*(-[a-z0-9]+)*$";
}

const APT_REPOSITORY_HANDLE_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&APT_REPOSITORY_HANDLE_REGEX);

/// Handle for a standard Proxmox APT repository: an [`APTRepoType`] (software) and an
/// [`APTRepoComponent`] (channel); `repo_type == None` is the host product's own channel.
/// Wire format is the historical kebab-case string. `FromStr` is byte-strict (lowercase only),
/// `Deserialize` lowercases first to mirror the open enums; `new()` lowercases `Unknown(_)`
/// payloads and debug-asserts they match the open-enum kebab shape so a programmatically built
/// handle round-trips through its own `Display`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct APTRepositoryHandle {
    repo_type: Option<APTRepoType>,
    component: APTRepoComponent,
}

/// Known component-name strings, longest first so suffix-matching picks the longest match.
const KNOWN_COMPONENTS: &[(&str, APTRepoComponent)] = &[
    ("no-subscription", APTRepoComponent::NoSubscription),
    ("enterprise", APTRepoComponent::Enterprise),
    ("staging", APTRepoComponent::Staging),
    ("test", APTRepoComponent::Test),
    ("main", APTRepoComponent::Main),
];

/// Non-host-product repo-type prefixes, longest first so `debian-security` beats `debian`.
/// Host products (`pve`, `pbs`, ...) intentionally absent; their wire form has no prefix.
const KNOWN_NONHOST_REPO_TYPES: &[(&str, APTRepoType)] = &[
    ("debian-backports", APTRepoType::DebianBackports),
    ("debian-security", APTRepoType::DebianSecurity),
    ("ceph-tentacle", APTRepoType::CephTentacle),
    ("ceph-squid", APTRepoType::CephSquid),
    ("pbs-client", APTRepoType::PbsClient),
    ("debian", APTRepoType::Debian),
];

impl APTRepositoryHandle {
    /// Collapses host-product `repo_type` to `None` to match the wire-round-tripped form.
    pub fn new(repo_type: Option<APTRepoType>, mut component: APTRepoComponent) -> Self {
        let repo_type = match repo_type {
            Some(rt) if rt.is_host_product() => None,
            Some(APTRepoType::Unknown(mut payload)) => {
                payload.make_ascii_lowercase();
                debug_assert!(
                    is_apt_open_enum_token(&payload),
                    "APTRepoType::Unknown payload {payload:?} is not a valid open-enum kebab token",
                );
                Some(APTRepoType::Unknown(payload))
            }
            other => other,
        };
        if let APTRepoComponent::Unknown(payload) = &mut component {
            payload.make_ascii_lowercase();
            debug_assert!(
                is_apt_open_enum_token(payload),
                "APTRepoComponent::Unknown payload {payload:?} is not a valid open-enum kebab token",
            );
        }
        Self {
            repo_type,
            component,
        }
    }

    /// `None` for host-product channels (no prefix on the wire).
    pub fn repo_type(&self) -> Option<&APTRepoType> {
        self.repo_type.as_ref()
    }

    /// The release channel.
    pub fn component(&self) -> &APTRepoComponent {
        &self.component
    }

    const fn host(component: APTRepoComponent) -> Self {
        Self {
            repo_type: None,
            component,
        }
    }

    const fn standalone(repo_type: APTRepoType, component: APTRepoComponent) -> Self {
        Self {
            repo_type: Some(repo_type),
            component,
        }
    }

    pub const ENTERPRISE: Self = Self::host(APTRepoComponent::Enterprise);
    pub const NO_SUBSCRIPTION: Self = Self::host(APTRepoComponent::NoSubscription);
    pub const TEST: Self = Self::host(APTRepoComponent::Test);
    pub const CEPH_SQUID_ENTERPRISE: Self =
        Self::standalone(APTRepoType::CephSquid, APTRepoComponent::Enterprise);
    pub const CEPH_SQUID_NO_SUBSCRIPTION: Self =
        Self::standalone(APTRepoType::CephSquid, APTRepoComponent::NoSubscription);
    pub const CEPH_SQUID_TEST: Self =
        Self::standalone(APTRepoType::CephSquid, APTRepoComponent::Test);
    pub const CEPH_TENTACLE_ENTERPRISE: Self =
        Self::standalone(APTRepoType::CephTentacle, APTRepoComponent::Enterprise);
    pub const CEPH_TENTACLE_NO_SUBSCRIPTION: Self =
        Self::standalone(APTRepoType::CephTentacle, APTRepoComponent::NoSubscription);
    pub const CEPH_TENTACLE_TEST: Self =
        Self::standalone(APTRepoType::CephTentacle, APTRepoComponent::Test);
}

impl Display for APTRepositoryHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.repo_type {
            None => write!(f, "{}", self.component),
            Some(rt) if rt.is_host_product() => write!(f, "{}", self.component),
            Some(rt) => write!(f, "{}-{}", rt, self.component),
        }
    }
}

impl std::str::FromStr for APTRepositoryHandle {
    type Err = APTRepositoryError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Match the API schema regex on every entry path so non-router callers can't construct
        // handles that would surface as garbage on-disk component names.
        if !APT_REPOSITORY_HANDLE_REGEX.is_match(s) {
            return Err(APTRepositoryError::new(format!(
                "invalid APT repository handle: '{s}'"
            )));
        }
        // Greedy suffix match: longest known component wins so "ceph-future-enterprise" splits as
        // {Unknown("ceph-future"), Enterprise} when the repo_type is not yet known.
        for (comp_str, component) in KNOWN_COMPONENTS {
            let Some(prefix) = s.strip_suffix(comp_str) else {
                continue;
            };
            let prefix = match prefix.strip_suffix('-') {
                Some(p) if !p.is_empty() => p,
                Some(_) => continue,
                None if prefix.is_empty() => return Ok(Self::host(component.clone())),
                None => continue,
            };
            let repo_type = KNOWN_NONHOST_REPO_TYPES
                .iter()
                .find_map(|(s, rt)| (*s == prefix).then(|| rt.clone()))
                .unwrap_or_else(|| APTRepoType::Unknown(prefix.to_string()));
            return Ok(Self::standalone(repo_type, component.clone()));
        }
        // No known component suffix; opaque fallback. The regex above already constrained `s`.
        Ok(Self::host(APTRepoComponent::Unknown(s.to_string())))
    }
}

proxmox_serde::forward_serialize_to_display!(APTRepositoryHandle);

impl<'de> Deserialize<'de> for APTRepositoryHandle {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut s = String::deserialize(deserializer)?;
        s.make_ascii_lowercase();
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl proxmox_schema::ApiType for APTRepositoryHandle {
    const API_SCHEMA: proxmox_schema::Schema =
        proxmox_schema::StringSchema::new("Handle referencing a standard APT repository.")
            .format(&APT_REPOSITORY_HANDLE_FORMAT)
            .schema();
}

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

#[api(
    properties: {
        name: {
            type: String,
            format: &PACKAGE_NAME_FORMAT,
        },
    },
)]
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Pin the historic wire strings perl/UI consumers embed; their bytes are API contract.
    #[test]
    fn handle_known_wire_round_trip() {
        for (wire, expected) in [
            ("enterprise", APTRepositoryHandle::ENTERPRISE),
            ("no-subscription", APTRepositoryHandle::NO_SUBSCRIPTION),
            ("test", APTRepositoryHandle::TEST),
            (
                "ceph-squid-enterprise",
                APTRepositoryHandle::CEPH_SQUID_ENTERPRISE,
            ),
            (
                "ceph-squid-no-subscription",
                APTRepositoryHandle::CEPH_SQUID_NO_SUBSCRIPTION,
            ),
            ("ceph-squid-test", APTRepositoryHandle::CEPH_SQUID_TEST),
        ] {
            let parsed: APTRepositoryHandle = wire
                .parse()
                .unwrap_or_else(|e| panic!("parsing known wire string {wire:?} failed: {e}"));
            assert_eq!(parsed, expected, "parse({wire:?})");
            assert_eq!(parsed.to_string(), wire, "render({expected:?})");
        }

        // Property: KNOWN_NONHOST_REPO_TYPES x KNOWN_COMPONENTS round-trips bytewise.
        for (prefix, repo_type) in KNOWN_NONHOST_REPO_TYPES {
            assert!(
                !repo_type.is_host_product(),
                "{repo_type:?} is classified as host product but listed in KNOWN_NONHOST_REPO_TYPES",
            );
            assert_eq!(
                repo_type.to_string(),
                *prefix,
                "Display for {repo_type:?} drifted from KNOWN_NONHOST_REPO_TYPES"
            );
            for (comp_str, component) in KNOWN_COMPONENTS {
                let wire = format!("{prefix}-{comp_str}");
                let parsed: APTRepositoryHandle = wire
                    .parse()
                    .unwrap_or_else(|e| panic!("parsing {wire:?} failed: {e}"));
                assert_eq!(parsed.repo_type.as_ref(), Some(repo_type), "wire={wire}");
                assert_eq!(&parsed.component, component, "wire={wire}");
                assert_eq!(parsed.to_string(), wire, "round-trip {wire:?}");
            }
        }
    }

    /// Unknown prefix + known component: structure preserved so old clients still see the channel.
    #[test]
    fn handle_unknown_repo_type_round_trip() {
        let wire = "ceph-future-enterprise";
        let parsed: APTRepositoryHandle = wire.parse().unwrap();
        assert_eq!(
            parsed,
            APTRepositoryHandle::standalone(
                APTRepoType::Unknown("ceph-future".into()),
                APTRepoComponent::Enterprise,
            )
        );
        assert_eq!(parsed.to_string(), wire);
    }

    /// No recognized component suffix: fully-opaque round-trip; never mangle bytes.
    #[test]
    fn handle_fully_opaque_round_trip() {
        let wire = "some-future-thing";
        let parsed: APTRepositoryHandle = wire.parse().unwrap();
        assert_eq!(
            parsed,
            APTRepositoryHandle::host(APTRepoComponent::Unknown(wire.into()))
        );
        assert_eq!(parsed.to_string(), wire);
    }

    /// A host-product `repo_type` collapses to `None`; wire form has no product prefix.
    #[test]
    fn handle_host_product_repo_type_collapses() {
        let h = APTRepositoryHandle::new(Some(APTRepoType::Pve), APTRepoComponent::Enterprise);
        assert_eq!(h.repo_type, None);
        assert_eq!(h.to_string(), "enterprise");
    }

    /// An empty wire string is not a valid handle.
    #[test]
    fn handle_empty_wire_string_rejected() {
        assert!("".parse::<APTRepositoryHandle>().is_err());
    }

    /// FromStr enforces the API-schema regex so direct callers can't build garbage handles.
    #[test]
    fn handle_invalid_wire_strings_rejected() {
        for bad in [
            "-test",
            "test-",
            " ",
            "test test",
            "Test",
            "ENTERPRISE",
            "no--sub",
        ] {
            assert!(
                bad.parse::<APTRepositoryHandle>().is_err(),
                "expected {bad:?} to be rejected"
            );
        }
    }

    /// `new()` collapses every host-product variant to the bare host-channel handle.
    #[test]
    fn handle_new_collapses_all_host_products() {
        for hp in [
            APTRepoType::Pve,
            APTRepoType::Pbs,
            APTRepoType::Pdm,
            APTRepoType::Pmg,
        ] {
            let via_new = APTRepositoryHandle::new(Some(hp.clone()), APTRepoComponent::Enterprise);
            assert_eq!(via_new, APTRepositoryHandle::ENTERPRISE);
            assert_eq!(via_new.to_string(), "enterprise");
            assert_eq!(via_new.repo_type, None, "{hp:?} did not collapse");
        }
    }

    /// Mixed-case wire input lowercases before matching; no split between known and Unknown.
    #[test]
    fn open_enums_lowercase_normalize() {
        let cases: &[(&str, DebianCodename)] = &[
            ("BOOKWORM", DebianCodename::Bookworm),
            ("Trixie", DebianCodename::Trixie),
            ("trixie", DebianCodename::Trixie),
        ];
        for (wire, expected) in cases {
            let json = format!("\"{wire}\"");
            let parsed: DebianCodename = serde_json::from_str(&json).unwrap();
            assert_eq!(&parsed, expected, "{wire}");
            assert!(
                parsed < DebianCodename::Unknown("future".into()),
                "Unknown sorts above every known codename"
            );
        }
        let upper: DebianCodename = serde_json::from_str("\"FUTURE-NAME\"").unwrap();
        let lower: DebianCodename = serde_json::from_str("\"future-name\"").unwrap();
        assert_eq!(upper, lower);
        assert_eq!(upper, DebianCodename::Unknown("future-name".into()));

        let hp: HostProduct = serde_json::from_str("\"PVE\"").unwrap();
        assert_eq!(hp, HostProduct::Pve);
        let rt: APTRepoType = serde_json::from_str("\"CEPH-SQUID\"").unwrap();
        assert_eq!(rt, APTRepoType::CephSquid);
        let rc: APTRepoComponent = serde_json::from_str("\"NO-SUBSCRIPTION\"").unwrap();
        assert_eq!(rc, APTRepoComponent::NoSubscription);

        let h: APTRepositoryHandle = serde_json::from_str("\"Enterprise\"").unwrap();
        assert_eq!(h, APTRepositoryHandle::ENTERPRISE);
        let h: APTRepositoryHandle =
            serde_json::from_str("\"CEPH-SQUID-No-Subscription\"").unwrap();
        assert_eq!(h, APTRepositoryHandle::CEPH_SQUID_NO_SUBSCRIPTION);
    }

    /// `new()` lowercases `Unknown(_)` payloads so programmatic + wire paths produce the same form.
    #[test]
    fn handle_new_normalizes_unknown_payloads() {
        let mixed = APTRepositoryHandle::new(
            Some(APTRepoType::Unknown("CEPH-FUTURE".into())),
            APTRepoComponent::Enterprise,
        );
        let lower = APTRepositoryHandle::new(
            Some(APTRepoType::Unknown("ceph-future".into())),
            APTRepoComponent::Enterprise,
        );
        assert_eq!(mixed, lower);
        assert_eq!(mixed.to_string(), "ceph-future-enterprise");
        let round: APTRepositoryHandle = mixed.to_string().parse().unwrap();
        assert_eq!(round, mixed);

        let mixed_comp = APTRepositoryHandle::new(None, APTRepoComponent::Unknown("WEIRD".into()));
        let lower_comp = APTRepositoryHandle::new(None, APTRepoComponent::Unknown("weird".into()));
        assert_eq!(mixed_comp, lower_comp);
    }

    /// Greedy suffix match: longest known component wins (no-subscription beats anything shorter).
    #[test]
    fn handle_longest_component_wins() {
        let parsed: APTRepositoryHandle = "ceph-squid-no-subscription".parse().unwrap();
        assert_eq!(parsed, APTRepositoryHandle::CEPH_SQUID_NO_SUBSCRIPTION);
    }

    /// JSON round-trip via serde, which is what the REST API uses.
    #[test]
    fn handle_json_round_trip() {
        let h = APTRepositoryHandle::CEPH_SQUID_ENTERPRISE;
        let json = serde_json::to_string(&h).unwrap();
        assert_eq!(json, "\"ceph-squid-enterprise\"");
        let back: APTRepositoryHandle = serde_json::from_str(&json).unwrap();
        assert_eq!(back, h);
    }

    /// Wire-bytes round-trip + the TryFrom / Deserialize miss-handling split.
    #[test]
    fn debian_codename_round_trip() {
        for (wire, expected) in [
            ("trixie", DebianCodename::Trixie),
            ("bookworm", DebianCodename::Bookworm),
            ("forky", DebianCodename::Forky),
        ] {
            assert_eq!(DebianCodename::try_from(wire).unwrap(), expected);
            assert_eq!(expected.to_string(), wire);
        }
        // Strict TryFrom rejects unknown; Deserialize accepts forward-compat Unknown.
        assert!(DebianCodename::try_from("zürich").is_err());
        let parsed: DebianCodename = serde_json::from_str("\"zurichish\"").unwrap();
        assert_eq!(parsed, DebianCodename::Unknown("zurichish".into()));
        assert_eq!(parsed.to_string(), "zurichish");
    }

    /// HostProduct must serialize to the expected lowercase strings.
    #[test]
    fn host_product_round_trip() {
        for (wire, expected) in [
            ("pve", HostProduct::Pve),
            ("pbs", HostProduct::Pbs),
            ("pdm", HostProduct::Pdm),
            ("pmg", HostProduct::Pmg),
        ] {
            let json = serde_json::to_string(&expected).unwrap();
            assert_eq!(json, format!("\"{wire}\""));
            let back: HostProduct = serde_json::from_str(&json).unwrap();
            assert_eq!(back, expected);
            assert_eq!(expected.as_str(), wire);
        }
        // Unknown forward-compat fallback.
        let parsed: HostProduct = serde_json::from_str("\"future-product\"").unwrap();
        assert_eq!(parsed, HostProduct::Unknown("future-product".into()));
    }

    /// Drift guard: `_force_exhaustive_*` compile-fails on a new variant so the round-trip list
    /// (and KNOWN_NONHOST_REPO_TYPES / STANDARD_REPOS for `APTRepoType`) gets extended too.
    #[test]
    fn open_enum_variants_round_trip() {
        // --- DebianCodename ---
        fn _force_exhaustive_codename(v: DebianCodename) {
            match v {
                DebianCodename::Lenny
                | DebianCodename::Squeeze
                | DebianCodename::Wheezy
                | DebianCodename::Jessie
                | DebianCodename::Stretch
                | DebianCodename::Buster
                | DebianCodename::Bullseye
                | DebianCodename::Bookworm
                | DebianCodename::Trixie
                | DebianCodename::Forky
                | DebianCodename::Duke
                | DebianCodename::Unknown(_) => {}
            }
        }
        for v in [
            DebianCodename::Lenny,
            DebianCodename::Squeeze,
            DebianCodename::Wheezy,
            DebianCodename::Jessie,
            DebianCodename::Stretch,
            DebianCodename::Buster,
            DebianCodename::Bullseye,
            DebianCodename::Bookworm,
            DebianCodename::Trixie,
            DebianCodename::Forky,
            DebianCodename::Duke,
        ] {
            let json = serde_json::to_string(&v).unwrap();
            let back: DebianCodename = serde_json::from_str(&json).unwrap();
            assert_eq!(back, v, "DebianCodename round-trip failed via {json}");
        }

        // --- HostProduct ---
        fn _force_exhaustive_host(v: HostProduct) {
            match v {
                HostProduct::Pve
                | HostProduct::Pbs
                | HostProduct::Pdm
                | HostProduct::Pmg
                | HostProduct::Unknown(_) => {}
            }
        }
        for v in [
            HostProduct::Pve,
            HostProduct::Pbs,
            HostProduct::Pdm,
            HostProduct::Pmg,
        ] {
            let json = serde_json::to_string(&v).unwrap();
            let back: HostProduct = serde_json::from_str(&json).unwrap();
            assert_eq!(back, v);
        }

        // --- APTRepoType ---
        fn _force_exhaustive_repo_type(v: APTRepoType) {
            match v {
                APTRepoType::Pve
                | APTRepoType::Pbs
                | APTRepoType::PbsClient
                | APTRepoType::Pdm
                | APTRepoType::Pmg
                | APTRepoType::Debian
                | APTRepoType::DebianSecurity
                | APTRepoType::DebianBackports
                | APTRepoType::CephSquid
                | APTRepoType::CephTentacle
                | APTRepoType::Unknown(_) => {}
            }
        }
        for v in [
            APTRepoType::Pve,
            APTRepoType::Pbs,
            APTRepoType::PbsClient,
            APTRepoType::Pdm,
            APTRepoType::Pmg,
            APTRepoType::Debian,
            APTRepoType::DebianSecurity,
            APTRepoType::DebianBackports,
            APTRepoType::CephSquid,
            APTRepoType::CephTentacle,
        ] {
            let json = serde_json::to_string(&v).unwrap();
            let back: APTRepoType = serde_json::from_str(&json).unwrap();
            assert_eq!(back, v);
            if !v.is_host_product() {
                assert!(
                    KNOWN_NONHOST_REPO_TYPES.iter().any(|(_, rt)| rt == &v),
                    "non-host APTRepoType {v:?} missing from KNOWN_NONHOST_REPO_TYPES",
                );
            }
        }

        // Cross-check the kebab table against the Deserialize impl: catches typos that would
        // silently route a known wire string to Unknown(_) instead of the typed variant.
        for (kebab, expected) in KNOWN_NONHOST_REPO_TYPES {
            let parsed: APTRepoType = serde_json::from_str(&format!("\"{kebab}\"")).unwrap();
            assert_eq!(
                &parsed, expected,
                "kebab {kebab:?} did not Deserialize to {expected:?}",
            );
        }

        // --- APTRepoComponent ---
        fn _force_exhaustive_component(v: APTRepoComponent) {
            match v {
                APTRepoComponent::Enterprise
                | APTRepoComponent::NoSubscription
                | APTRepoComponent::Test
                | APTRepoComponent::Main
                | APTRepoComponent::Staging
                | APTRepoComponent::Unknown(_) => {}
            }
        }
        for v in [
            APTRepoComponent::Enterprise,
            APTRepoComponent::NoSubscription,
            APTRepoComponent::Test,
            APTRepoComponent::Main,
            APTRepoComponent::Staging,
        ] {
            let json = serde_json::to_string(&v).unwrap();
            let back: APTRepoComponent = serde_json::from_str(&json).unwrap();
            assert_eq!(back, v);
        }
    }

    fn std_repo(handle: APTRepositoryHandle, enabled: Option<bool>) -> APTStandardRepository {
        APTStandardRepository {
            handle,
            status: enabled,
            name: String::new(),
            description: String::new(),
        }
    }

    #[test]
    fn standard_repo_summary_buckets() {
        use APTRepoComponent::*;
        let on = Some(true);
        let off = Some(false);
        let none = None;
        let h = |rt: Option<APTRepoType>, c: APTRepoComponent| APTRepositoryHandle::new(rt, c);
        let repos = vec![
            std_repo(h(None, Enterprise), on),
            std_repo(h(None, NoSubscription), off), // off: must not flip flag
            std_repo(h(None, Test), none),          // unconfigured: ditto
            std_repo(h(Some(APTRepoType::CephTentacle), Enterprise), on),
            std_repo(h(Some(APTRepoType::CephSquid), Test), on),
            std_repo(h(Some(APTRepoType::Debian), Main), on), // ignored bucket
            std_repo(
                h(Some(APTRepoType::Unknown("ceph-future".into())), Enterprise),
                on,
            ),
            std_repo(
                h(None, APTRepoComponent::Unknown("future-channel".into())),
                on,
            ),
        ];

        let summary = APTStandardRepoSummary::from_repos(&repos);

        assert!(summary.has_enterprise);
        assert!(!summary.has_no_subscription);
        assert!(!summary.has_test);
        assert!(summary.has_ceph_enterprise);
        assert!(!summary.has_ceph_no_subscription);
        assert!(summary.has_ceph_test);
        assert_eq!(
            summary.unrecognized.len(),
            2,
            "ceph-future + future-channel"
        );
    }
}
