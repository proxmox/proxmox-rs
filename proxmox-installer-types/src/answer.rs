//! Defines API types for the answer file format used by proxmox-auto-installer.
//!
//! **NOTE**: New answer file properties must use kebab-case, but should allow
//! snake_case for backwards compatibility.
//!
//! TODO: Remove the snake_case'd variants in a future major version (e.g.
//! PVE 10).

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt::{self, Display},
    str::FromStr,
};

use proxmox_network_types::{
    MacAddress,
    bond::{BondXmitHashPolicy, LinuxBondMode},
    fqdn::Fqdn,
    ip_address::Cidr,
};

#[cfg(feature = "api-types")]
use proxmox_schema::{
    ApiType, IntegerSchema, NumberSchema, ObjectSchema, Schema, StringSchema, Updater, UpdaterType,
    api,
    api_types::{DISK_ARRAY_SCHEMA, PASSWORD_FORMAT},
};

#[cfg(feature = "api-types")]
type IpAddr = proxmox_network_types::ip_address::api_types::IpAddr;
#[cfg(not(feature = "api-types"))]
type IpAddr = std::net::IpAddr;

#[cfg(feature = "api-types")]
proxmox_schema::const_regex! {
    /// A unique two-letter country code, according to ISO 3166-1 (alpha-2).
    pub COUNTRY_CODE_REGEX = r"^[a-z]{2}$";

    /// A Proxmox subscription key, of the form `<product><socket?><level>-<10 hex>`.
    ///
    /// PVE keys carry a maximum-socket-count digit between the product letters and the
    /// level letter (`pve4b-...`); PBS and PMG do not. Level letters are c/b/s/p
    /// (Community/Basic/Standard/Premium).
    pub SUBSCRIPTION_KEY_REGEX = r"^(?:pve[0-9]+|pbs|pmg)[cbsp]-[0-9a-f]{10}$";
}

proxmox_schema::const_regex! {
    /// An email address using the regex for `<input type="email" />` elements
    /// as defined in the [HTML specification].
    /// Using that /should/ cover all possible cases that are encountered in the wild.
    ///
    /// It additionally checks whether the email our default email placeholder value.
    ///
    /// [HTML specification]: <https://html.spec.whatwg.org/multipage/input.html#valid-e-mail-address>
    pub EMAIL_REGEX = r"^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$";
}

/// Schema for an optional Proxmox subscription key.
#[cfg(feature = "api-types")]
pub const SUBSCRIPTION_KEY_SCHEMA: proxmox_schema::Schema =
    StringSchema::new("A Proxmox subscription key for the installed product.")
        .format(&proxmox_schema::ApiStringFormat::Pattern(
            &SUBSCRIPTION_KEY_REGEX,
        ))
        .min_length(11)
        .max_length(32)
        .schema();

/// Defines API types used by proxmox-fetch-answer, the first part of the
/// auto-installer.
pub mod fetch {
    use serde::{Deserialize, Serialize};

    #[cfg(feature = "api-types")]
    use proxmox_schema::api;

    use crate::SystemInfo;

    #[cfg_attr(feature = "api-types", api)]
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "kebab-case")]
    /// Metadata of the HTTP POST payload, such as schema version of the document.
    pub struct AnswerFetchDataSchema {
        /// major.minor version describing the schema version of this document, in a semanticy-version
        /// way.
        ///
        /// major: Incremented for incompatible/breaking API changes, e.g. removing an existing
        /// field.
        /// minor: Incremented when adding functionality in a backwards-compatible matter, e.g.
        /// adding a new field.
        pub version: String,
    }

    impl AnswerFetchDataSchema {
        const SCHEMA_VERSION: &str = "1.0";
    }

    impl Default for AnswerFetchDataSchema {
        fn default() -> Self {
            Self {
                version: Self::SCHEMA_VERSION.to_owned(),
            }
        }
    }

    #[cfg_attr(feature = "api-types", api(
            properties: {
                sysinfo: {
                    flatten: true,
                },
            },
    ))]
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "kebab-case")]
    /// Data sent in the body of POST request when retrieving the answer file via HTTP(S).
    ///
    /// NOTE: The format is versioned through `schema.version` (`$schema.version` in the
    /// resulting JSON), ensure you update it when this struct or any of its members gets modified.
    pub struct AnswerFetchData {
        /// Metadata for the answer file fetch payload
        // This field is prefixed by `$` on purpose, to indicate that it is document metadata and not
        // part of the actual content itself. (E.g. JSON Schema uses a similar naming scheme)
        #[serde(rename = "$schema")]
        pub schema: AnswerFetchDataSchema,
        /// Information about the running system, flattened into this structure directly.
        #[serde(flatten)]
        pub sysinfo: SystemInfo,
    }
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Clone, Deserialize, Debug, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// Top-level answer file structure, describing all possible options for an
/// automated installation.
pub struct AutoInstallerConfig {
    /// General target system options for setting up the system in an automated
    /// installation.
    pub global: GlobalOptions,
    /// Network configuration to set up inside the target installation.
    pub network: NetworkConfig,
    #[serde(rename = "disk-setup")]
    /// Disk configuration for the target installation.
    pub disks: DiskSetup,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional webhook to hit after a successful installation with information
    /// about the provisioned system.
    pub post_installation_webhook: Option<PostNotificationHookInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional one-time hook to run on the first boot into the newly provisioned
    /// system.
    pub first_boot: Option<FirstBootHookInfo>,
}

/// Machine root password schema.
#[cfg(feature = "api-types")]
pub const ROOT_PASSWORD_SCHEMA: proxmox_schema::Schema = StringSchema::new("Root Password.")
    .format(&PASSWORD_FORMAT)
    .min_length(8)
    .max_length(64)
    .schema();

#[cfg_attr(feature = "api-types", api(
    properties: {
        "root-ssh-keys": {
            type: Array,
            items: {
                description: "Public SSH key.",
                type: String,
            }
        },
        "subscription-key": {
            optional: true,
            schema: SUBSCRIPTION_KEY_SCHEMA,
        },
        fqdn: {
            type: FqdnConfigContainer,
            flatten: true,
        },
    },
))]
#[derive(Clone, Default, Deserialize, Debug, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// General target system options for setting up the system in an automated
/// installation.
pub struct GlobalOptions {
    /// Country to use for apt mirrors.
    pub country: String,
    /// Keyboard layout to set.
    pub keyboard: KeyboardLayout,
    /// Mail address for `root@pam`.
    pub mailto: String,
    /// Timezone to set on the new system.
    pub timezone: String,
    #[serde(alias = "root_password", skip_serializing_if = "Option::is_none")]
    /// Password to set for the `root` PAM account in plain text. Mutual
    /// exclusive with the `root-password-hashed` option.
    pub root_password: Option<String>,
    #[cfg_attr(feature = "legacy", serde(alias = "root_password_hashed"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Password to set for the `root` PAM account as hash, created using e.g.
    /// mkpasswd(8). Mutual exclusive with the `root-password` option.
    pub root_password_hashed: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "legacy", serde(alias = "reboot_on_error"))]
    /// Whether to reboot the machine if an error occurred during the
    /// installation.
    pub reboot_on_error: bool,
    #[serde(default)]
    #[cfg_attr(feature = "legacy", serde(alias = "reboot_mode"))]
    /// Action to take after the installation completed successfully.
    pub reboot_mode: RebootMode,
    #[serde(default)]
    #[cfg_attr(feature = "legacy", serde(alias = "root_ssh_keys"))]
    /// Public SSH keys to set up for the `root` PAM account.
    pub root_ssh_keys: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "legacy", serde(alias = "subscription_key"))]
    /// Optional Proxmox subscription key to apply to the installed system on
    /// first boot. Restricted to PVE, PBS and PMG keys.
    pub subscription_key: Option<String>,
    /// FQDN to set for the installed system.
    #[serde(flatten)]
    pub fqdn: FqdnConfigContainer,
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Copy, Clone, Deserialize, Serialize, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// Action to take after the installation completed successfully.
pub enum RebootMode {
    #[default]
    /// Reboot the machine.
    Reboot,
    /// Power off and halt the machine.
    PowerOff,
}

serde_plain::derive_fromstr_from_deserialize!(RebootMode);

#[derive(Clone, Deserialize, Debug, Serialize, PartialEq)]
#[cfg_attr(feature = "api-types", derive(Updater))]
#[serde(
    untagged,
    expecting = "either a fully-qualified domain name or extendend configuration for usage with DHCP must be specified"
)]
/// Allow the user to either set the FQDN of the installation to either some
/// fixed value or retrieve it dynamically via e.g.DHCP.
pub enum FqdnConfig {
    /// Sets the FQDN to the exact value.
    Simple(Fqdn),
    /// Extended configuration, e.g. to use hostname and domain from DHCP.
    FromDhcp(FqdnFromDhcpConfig),
}

impl Default for FqdnConfig {
    fn default() -> Self {
        Self::FromDhcp(FqdnFromDhcpConfig::default())
    }
}

impl FqdnConfig {
    /// Constructs a new "simple" FQDN configuration, i.e. a fixed hostname.
    pub fn simple<S: Into<String>>(fqdn: S) -> Result<Self> {
        Ok(Self::Simple(
            fqdn.into()
                .parse::<Fqdn>()
                .map_err(|err| anyhow!("{err}"))?,
        ))
    }

    /// Constructs an extended FQDN configuration, in particular instructing the
    /// auto-installer to use the FQDN from DHCP lease information.
    pub fn from_dhcp(domain: Option<String>) -> Self {
        Self::FromDhcp(FqdnFromDhcpConfig {
            source: FqdnSourceMode::FromDhcp,
            domain,
        })
    }
}

#[cfg(feature = "api-types")]
impl ApiType for FqdnConfigContainer {
    const API_SCHEMA: Schema = ObjectSchema::new(
        "Either a FQDN as string or an object describing the retrieval method in an 'fqdn' field.",
        &[],
    )
    .additional_properties(true)
    .schema();
}

/// This is a wrapping container for the `fqdn` field. The field does not fit into our supported
/// schema system and thus, from the schema point of view, we use an empty
/// `additionalProperties: true` object schema for it.
#[derive(Clone, Default, Deserialize, Debug, Serialize, PartialEq)]
pub struct FqdnConfigContainer {
    /// FQDN to set for the installed system.
    pub fqdn: FqdnConfig,
}

impl From<FqdnConfig> for FqdnConfigContainer {
    fn from(fqdn: FqdnConfig) -> Self {
        Self { fqdn }
    }
}

impl AsRef<FqdnConfig> for FqdnConfigContainer {
    fn as_ref(&self) -> &FqdnConfig {
        &self.fqdn
    }
}

impl From<FqdnConfigContainer> for FqdnConfig {
    fn from(fqdn: FqdnConfigContainer) -> Self {
        fqdn.fqdn
    }
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Clone, Default, Deserialize, Debug, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// Extended configuration for retrieving the FQDN from external sources.
pub struct FqdnFromDhcpConfig {
    /// Source to gather the FQDN from.
    #[serde(default)]
    pub source: FqdnSourceMode,
    /// Domain to use if none is received via DHCP.
    #[serde(default, deserialize_with = "deserialize_non_empty_string_maybe")]
    pub domain: Option<String>,
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Clone, Deserialize, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// Describes the source to retrieve the FQDN of the installation.
pub enum FqdnSourceMode {
    #[default]
    /// Use the FQDN as provided by the DHCP server, if any.
    FromDhcp,
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Clone, Deserialize, Debug, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// Configuration for the post-installation hook, which runs after an
/// installation has completed successfully.
pub struct PostNotificationHookInfo {
    /// URL to send a POST request to
    pub url: String,
    /// SHA256 cert fingerprint if certificate pinning should be used.
    #[serde(skip_serializing_if = "Option::is_none", alias = "cert_fingerprint")]
    pub cert_fingerprint: Option<String>,
    /// Optional opaque authentication token. The post-hook executor sends
    /// this back to the configured URL as a top-level "token" field so the
    /// receiving side can authenticate the callback.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "legacy", serde(alias = "auth_token"))]
    pub auth_token: Option<String>,
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Clone, Deserialize, Debug, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// Possible sources for the optional first-boot hook script/executable file.
pub enum FirstBootHookSourceMode {
    /// Fetch the executable file from an URL, specified in the parent.
    FromUrl,
    /// The executable file has been baked into the ISO at a known location,
    /// and should be retrieved from there.
    FromIso,
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Clone, Default, Deserialize, Debug, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// Possible orderings for the `proxmox-first-boot` systemd service.
///
/// Determines the final value of `Unit.Before` and `Unit.Wants` in the service
/// file.
// Must be kept in sync with Proxmox::Install::Config and the service files in the
// proxmox-first-boot package.
pub enum FirstBootHookServiceOrdering {
    /// Needed for bringing up the network itself, runs before any networking is attempted.
    BeforeNetwork,
    /// Network needs to be already online, runs after networking was brought up.
    NetworkOnline,
    /// Runs after the system has successfully booted up completely.
    #[default]
    FullyUp,
}

impl FirstBootHookServiceOrdering {
    /// Maps the enum to the appropriate systemd target name, without the '.target' suffix.
    pub fn as_systemd_target_name(&self) -> &str {
        match self {
            FirstBootHookServiceOrdering::BeforeNetwork => "network-pre",
            FirstBootHookServiceOrdering::NetworkOnline => "network-online",
            FirstBootHookServiceOrdering::FullyUp => "multi-user",
        }
    }
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Clone, Deserialize, Debug, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// Describes from where to fetch the first-boot hook script, either being baked into the ISO or
/// from a URL.
pub struct FirstBootHookInfo {
    /// Mode how to retrieve the first-boot executable file, either from an URL or from the ISO if
    /// it has been baked-in.
    pub source: FirstBootHookSourceMode,
    /// Determines the service order when the hook will run on first boot.
    #[serde(default)]
    pub ordering: FirstBootHookServiceOrdering,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Retrieve the post-install script from a URL, if source == "from-url".
    pub url: Option<String>,
    /// SHA256 cert fingerprint if certificate pinning should be used, if source == "from-url".
    #[cfg_attr(feature = "legacy", serde(alias = "cert_fingerprint"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cert_fingerprint: Option<String>,
}

#[cfg_attr(feature = "api-types", api(
    properties: {
        mapping: {
            type: Object,
            properties: {},
            additional_properties: true,
        }
    },
))]
#[derive(Clone, Deserialize, Debug, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// Options controlling the behaviour of the network interface pinning (by
/// creating appropriate systemd.link files) during the installation.
pub struct NetworkInterfacePinningOptionsAnswer {
    /// Whether interfaces should be pinned during the installation.
    pub enabled: bool,
    /// Maps MAC address to custom name
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub mapping: HashMap<MacAddress, String>,
}

#[cfg_attr(feature = "api-types", api(
    properties: {
        members: {
            description: "List of interface MAC addresses to add as bond members.",
            type: Array,
            items: {
                type: MacAddress,
            },
        },
    },
))]
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// Options for creating a bond for the management interface with the given
/// physical interfaces and mode.
pub struct NetworkBondOptions {
    /// List of interface MAC addresses to add as bond members.
    pub members: Vec<MacAddress>,
    /// Mode for the bond to set.
    pub mode: LinuxBondMode,
    /// Bond hash policy. Only used with '802.3ad' and 'balance-xor' mode.
    #[serde(default = "NetworkBondOptions::default_hash_policy")]
    pub hash_policy: BondXmitHashPolicy,
    /// Primary bond interface. Only used with 'active-backup' mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_interface: Option<MacAddress>,
}

impl NetworkBondOptions {
    /// List of all available bond modes, ordered by their mode number in the Linux kernel.
    pub const MODES: &[LinuxBondMode] = {
        use LinuxBondMode::*;
        &[
            BalanceRr,
            ActiveBackup,
            BalanceXor,
            Broadcast,
            Ieee802_3ad,
            BalanceTlb,
            BalanceAlb,
        ]
    };

    /// List of all available hash policies.
    pub const HASH_POLICIES: &[BondXmitHashPolicy] = {
        use BondXmitHashPolicy::*;
        &[Layer2, Layer2_3, Layer3_4]
    };

    /// Returns the default hash policy for new bonds.
    const fn default_hash_policy() -> BondXmitHashPolicy {
        BondXmitHashPolicy::Layer2
    }

    /// Does some basic checks on the options.
    ///
    /// This includes checks for:
    /// - There are at least two members
    /// - No member is given more than once
    /// - Whether the primary interface is applicable to the mode
    /// - Whether the primary interface (if given) is a member
    pub fn verify(&self) -> Result<()> {
        if self.members.len() < 2 {
            bail!(
                "bond setup requires at least 2 interfaces, got {}",
                self.members.len()
            );
        }

        let mut seen = HashSet::new();
        if let Some(duplicate) = self.members.iter().find(|mac| !seen.insert(*mac)) {
            bail!("bond member {duplicate} is given more than once");
        }

        if let Some(primary_if) = &self.primary_interface {
            if self.mode != LinuxBondMode::ActiveBackup {
                bail!("primary bond interface can only be set in active-backup mode");
            }

            if !self.members.contains(primary_if) {
                bail!(
                    "primary bond interface {primary_if} must be a member of the bond (members: {})",
                    self.members
                        .iter()
                        .fold(String::new(), |acc, mac| format!("{acc}, {mac}"))
                        .trim_start_matches(", ")
                );
            }
        }

        Ok(())
    }
}

impl Default for NetworkBondOptions {
    fn default() -> Self {
        Self {
            members: Vec::new(),
            mode: LinuxBondMode::ActiveBackup,
            hash_policy: Self::default_hash_policy(),
            primary_interface: None,
        }
    }
}

#[cfg_attr(feature = "api-types", api(
    properties: {
        vid: {
            type: Integer,
            // 0 and 4095 are reserved, 1 is the untagged default PVID of VLAN-aware bridges on
            // Linux.
            minimum: 2,
            maximum: 4094,
        }
    },
))]
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// Options for setting up a VLAN configuration on the management interface.
pub struct NetworkVlanOptions {
    /// VLAN tag to configure.
    pub vid: u16,
}

impl NetworkVlanOptions {
    /// Verifies whether the VLAN configuration is valid.
    pub fn verify(&self) -> Result<()> {
        if self.vid < 2 || self.vid > 4094 {
            bail!(
                "VLAN tag must be between 2 and 4094 inclusive, got {}",
                self.vid
            );
        }

        Ok(())
    }
}

#[cfg_attr(feature = "api-types", api(
    properties: {
        filter: {
            type: Object,
            properties: {},
            additional_properties: true,
        }
    },
))]
#[derive(Clone, Deserialize, Debug, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// Static network configuration given by the user.
pub struct NetworkConfigFromAnswer {
    /// CIDR of the machine.
    pub cidr: Cidr,
    /// DNS nameserver host to use.
    pub dns: IpAddr,
    /// Gateway to set.
    pub gateway: IpAddr,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    /// Filter for network devices, to select a specific management interface.
    pub filter: BTreeMap<String, String>,
    /// Controls network interface pinning behaviour during installation.
    /// Off by default. Allowed for both `from-dhcp` and `from-answer` modes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface_name_pinning: Option<NetworkInterfacePinningOptionsAnswer>,
    /// If set, sets up a Linux bond for the management interface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bond: Option<NetworkBondOptions>,
    /// If set, configures the given VLAN for the management interface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlan: Option<NetworkVlanOptions>,
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Clone, Deserialize, Debug, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// Use the network configuration received from the DHCP server.
pub struct NetworkConfigFromDhcp {
    /// Controls network interface pinning behaviour during installation.
    /// Off by default. Allowed for both `from-dhcp` and `from-answer` modes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface_name_pinning: Option<NetworkInterfacePinningOptionsAnswer>,
    /// If set, sets up a Linux bond for the management interface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bond: Option<NetworkBondOptions>,
}

#[cfg_attr(feature = "api-types", api(
    "id-property": "source",
    "id-schema": {
        type: String,
        description: "'from-dhcp' or 'from-answer'",
    }
))]
#[derive(Clone, Deserialize, Debug, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, tag = "source")]
/// Network configuration to set up inside the target installation.
/// It can either be given statically or taken from the DHCP lease.
pub enum NetworkConfig {
    /// Use the configuration from the DHCP lease.
    FromDhcp(NetworkConfigFromDhcp),
    /// Static configuration to apply.
    FromAnswer(NetworkConfigFromAnswer),
}

impl NetworkConfig {
    /// Returns the network interface pinning option answer, if any.
    pub fn interface_name_pinning(&self) -> Option<&NetworkInterfacePinningOptionsAnswer> {
        match self {
            Self::FromDhcp(dhcp) => dhcp.interface_name_pinning.as_ref(),
            Self::FromAnswer(answer) => answer.interface_name_pinning.as_ref(),
        }
    }

    /// Returns the management network Linux bond options, if any.
    pub const fn bond(&self) -> Option<&NetworkBondOptions> {
        match self {
            Self::FromDhcp(dhcp) => dhcp.bond.as_ref(),
            Self::FromAnswer(answer) => answer.bond.as_ref(),
        }
    }

    /// Returns the management network VLAN options, if any.
    ///
    /// Only available with a static configuration, with DHCP the installed system uses the VLAN of
    /// the installation environment.
    pub const fn vlan(&self) -> Option<&NetworkVlanOptions> {
        match self {
            Self::FromDhcp(_) => None,
            Self::FromAnswer(answer) => answer.vlan.as_ref(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "api-types", derive(UpdaterType))]
#[serde(rename_all = "kebab-case", tag = "filesystem")]
/// Filesystem-specific options to set on the root disk.
pub enum FilesystemOptions {
    /// Ext4-specific options.
    Ext4(LvmOptions),
    /// Ext4-specific options.
    Xfs(LvmOptions),
    /// Btrfs-specific options.
    Btrfs(BtrfsOptions),
    /// ZFS-specific options.
    Zfs(ZfsOptions),
}

impl FilesystemOptions {
    /// Returns the accompanying [`FilesystemType`] for this configuration.
    pub fn to_type(&self) -> FilesystemType {
        match self {
            FilesystemOptions::Ext4(_) => FilesystemType::Ext4,
            FilesystemOptions::Xfs(_) => FilesystemType::Xfs,
            FilesystemOptions::Zfs(ZfsOptions { raid, .. }) => {
                FilesystemType::Zfs(raid.unwrap_or_default())
            }
            FilesystemOptions::Btrfs(BtrfsOptions { raid, .. }) => {
                FilesystemType::Btrfs(raid.unwrap_or_default())
            }
        }
    }
}

#[cfg(feature = "api-types")]
impl ApiType for FilesystemOptions {
    // FIXME: proxmox-schema can not correctly differentiate between different
    // enums in struct members with the same name.
    const API_SCHEMA: Schema = ObjectSchema::new(
        "Filesystem-specific options to set on the root disk.",
        &[
            (
                "arc-max",
                true,
                &IntegerSchema::new("Maximum ARC size that ZFS should use, in MiB")
                    .minimum(64)
                    .schema(),
            ),
            (
                "ashift",
                true,
                &IntegerSchema::new("`ashift` value to create the zpool with.")
                    .minimum(9)
                    .maximum(16)
                    .default(12)
                    .schema(),
            ),
            (
                "copies",
                true,
                &IntegerSchema::new("`copies` value to create the zpool with.")
                    .minimum(1)
                    .maximum(3)
                    .default(1)
                    .schema(),
            ),
            ("filesystem", false, &Filesystem::API_SCHEMA),
            (
                "hdsize",
                true,
                &NumberSchema::new("Size of the root disk to use, in GiB.")
                    .minimum(2.)
                    .schema(),
            ),
            (
                "maxfree",
                true,
                &NumberSchema::new(
                    "Minimum amount of free space to leave on the LVM volume group, in GiB.",
                )
                .minimum(0.)
                .schema(),
            ),
            (
                "maxroot",
                true,
                &NumberSchema::new("Maximum size of the `root` volume, in GiB.")
                    .minimum(2.)
                    .schema(),
            ),
            (
                "maxvz",
                true,
                &NumberSchema::new("Maximum size of the `data` volume, in GiB.")
                    .minimum(0.)
                    .schema(),
            ),
            (
                "minfree",
                true,
                &NumberSchema::new(
                    "Minimum amount of free space that should be left in the LVM volume group.",
                )
                .minimum(0.)
                .schema(),
            ),
            (
                "swapsize",
                true,
                &NumberSchema::new("Size of the swap volume, in GiB.")
                    .minimum(0.)
                    .schema(),
            ),
        ],
    )
    .additional_properties(true)
    .schema();
}

#[derive(Clone, Debug, Serialize)]
/// Defines the disks to use for the installation. Can either be a fixed list
/// of disk names or a dynamic filter list.
pub enum DiskSelection {
    /// Fixed list of disk names to use for the installation.
    Selection(Vec<String>),
    /// Select disks dynamically by filtering them by udev properties.
    Filter(BTreeMap<String, String>),
}

impl Display for DiskSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Selection(disks) => write!(f, "{}", disks.join(", ")),
            Self::Filter(map) => write!(
                f,
                "{}",
                map.iter()
                    .fold(String::new(), |acc, (k, v)| format!("{acc}{k}: {v}\n"))
                    .trim_end()
            ),
        }
    }
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Copy, Clone, Default, Deserialize, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
/// Whether the associated filters must all match for a device or if any one
/// is enough.
pub enum FilterMatch {
    /// Device must match any filter.
    #[default]
    Any,
    /// Device must match all given filters.
    All,
}

serde_plain::derive_fromstr_from_deserialize!(FilterMatch);

#[cfg_attr(feature = "api-types", api(
    properties: {
        "disk-list": {
            schema: DISK_ARRAY_SCHEMA,
        },
        filter: {
            type: Object,
            properties: {},
            additional_properties: true,
        }
    },
))]
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// Disk configuration for the target installation.
pub struct DiskSetup {
    /// Filesystem to use on the root disk.
    pub filesystem: Filesystem,
    #[serde(default)]
    #[cfg_attr(feature = "legacy", serde(alias = "disk_list"))]
    /// List of raw disk identifiers to use for the root filesystem.
    pub disk_list: Vec<String>,
    #[serde(default)]
    /// Filter against udev properties to select the disks for the installation,
    /// to allow dynamic selection of disks.
    pub filter: BTreeMap<String, String>,
    #[cfg_attr(feature = "legacy", serde(alias = "filter_match"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Set whether it is enough that any filter matches on a disk or all given
    /// filters must match to select a disk.
    pub filter_match: Option<FilterMatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// ZFS-specific filesystem options.
    pub zfs: Option<ZfsOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// LVM-specific filesystem options, when using ext4 or xfs as filesystem.
    pub lvm: Option<LvmOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Btrfs-specific filesystem options.
    pub btrfs: Option<BtrfsOptions>,
}

impl DiskSetup {
    /// Returns the concrete disk selection made in the setup.
    pub fn disk_selection(&self) -> Result<DiskSelection> {
        if self.disk_list.is_empty() && self.filter.is_empty() {
            bail!("Need either 'disk-list' or 'filter' set");
        }
        if !self.disk_list.is_empty() && !self.filter.is_empty() {
            bail!("Cannot use both, 'disk-list' and 'filter'");
        }

        if !self.disk_list.is_empty() {
            Ok(DiskSelection::Selection(self.disk_list.clone()))
        } else {
            Ok(DiskSelection::Filter(self.filter.clone()))
        }
    }

    /// Returns the concrete filesystem type and corresponding options selected
    /// in the setup.
    pub fn filesystem_details(&self) -> Result<FilesystemOptions> {
        let lvm_checks = || -> Result<()> {
            if self.zfs.is_some() || self.btrfs.is_some() {
                bail!("make sure only 'lvm' options are set");
            }
            if self.disk_list.len() > 1 {
                bail!("make sure to define only one disk for ext4 and xfs");
            }
            Ok(())
        };

        match self.filesystem {
            Filesystem::Xfs => {
                lvm_checks()?;
                Ok(FilesystemOptions::Xfs(self.lvm.unwrap_or_default()))
            }
            Filesystem::Ext4 => {
                lvm_checks()?;
                Ok(FilesystemOptions::Ext4(self.lvm.unwrap_or_default()))
            }
            Filesystem::Zfs => {
                if self.lvm.is_some() || self.btrfs.is_some() {
                    bail!("make sure only 'zfs' options are set");
                }
                match self.zfs {
                    None | Some(ZfsOptions { raid: None, .. }) => {
                        bail!("ZFS raid level 'zfs.raid' must be set");
                    }
                    Some(opts) => {
                        // safe: the previous arm already covered raid = None
                        let raid = opts.raid.unwrap();
                        // a filter-based selection resolves disks only at install time;
                        // only an explicit disk-list is checkable here
                        if !self.disk_list.is_empty() {
                            let n = self.disk_list.len();
                            if n < raid.min_disks() {
                                bail!(
                                    "ZFS {raid} needs at least {} disks, got {n}",
                                    raid.min_disks(),
                                );
                            }
                            if raid.requires_even_disks() && !n.is_multiple_of(2) {
                                bail!("ZFS {raid} needs an even number of disks, got {n}");
                            }
                        }
                        Ok(FilesystemOptions::Zfs(opts))
                    }
                }
            }
            Filesystem::Btrfs => {
                if self.zfs.is_some() || self.lvm.is_some() {
                    bail!("make sure only 'btrfs' options are set");
                }
                match self.btrfs {
                    None | Some(BtrfsOptions { raid: None, .. }) => {
                        bail!("Btrfs raid level 'btrfs.raid' must be set");
                    }
                    Some(opts) => {
                        // safe: the previous arm already covered raid = None
                        let raid = opts.raid.unwrap();
                        // a filter-based selection resolves disks only at install time;
                        // only an explicit disk-list is checkable here
                        if !self.disk_list.is_empty() {
                            let n = self.disk_list.len();
                            if n < raid.min_disks() {
                                bail!(
                                    "Btrfs {raid} needs at least {} disks, got {n}",
                                    raid.min_disks(),
                                );
                            }
                            if raid.requires_even_disks() && !n.is_multiple_of(2) {
                                bail!("Btrfs {raid} needs an even number of disks, got {n}");
                            }
                        }
                        Ok(FilesystemOptions::Btrfs(opts))
                    }
                }
            }
        }
    }
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Copy, Clone, Deserialize, Serialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
/// Available filesystem during installation.
pub enum Filesystem {
    /// Fourth extended filesystem
    Ext4,
    /// XFS
    Xfs,
    /// ZFS
    Zfs,
    /// Btrfs
    Btrfs,
}

impl From<FilesystemType> for Filesystem {
    fn from(value: FilesystemType) -> Self {
        match value {
            FilesystemType::Ext4 => Self::Ext4,
            FilesystemType::Xfs => Self::Xfs,
            FilesystemType::Zfs(_) => Self::Zfs,
            FilesystemType::Btrfs(_) => Self::Btrfs,
        }
    }
}

serde_plain::derive_display_from_serialize!(Filesystem);
serde_plain::derive_fromstr_from_deserialize!(Filesystem);

#[cfg_attr(feature = "api-types", api(
    properties: {
        raid: {
            type: ZfsRaidLevel,
            optional: true,
        },
        ashift: {
            type: Integer,
            minimum: 9,
            maximum: 16,
            default: 12,
            optional: true,
        },
        "arc-max": {
            type: Integer,
            // ZFS specifies 64 MiB as the absolute minimum.
            minimum: 64,
            optional: true,
        },
        checksum: {
            type: ZfsChecksumOption,
            optional: true,
        },
        compress: {
            type: ZfsChecksumOption,
            optional: true,
        },
        copies: {
            type: Integer,
            minimum: 1,
            maximum: 3,
            optional: true,
        },
        hdsize: {
            type: Number,
            minimum: 2.,
            optional: true,
        },
    },
), derive(Updater))]
#[derive(Clone, Copy, Default, Deserialize, Debug, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// ZFS-specific filesystem options.
pub struct ZfsOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// RAID level to use.
    pub raid: Option<ZfsRaidLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// `ashift` value to create the zpool with.
    pub ashift: Option<u32>,
    #[cfg_attr(feature = "legacy", serde(alias = "arc_max"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Maximum ARC size that ZFS should use, in MiB.
    pub arc_max: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Checksumming algorithm to create the zpool with.
    pub checksum: Option<ZfsChecksumOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Compression algorithm to set on the zpool.
    pub compress: Option<ZfsCompressOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// `copies` value to create the zpool with.
    pub copies: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Size of the root disk to use, can be used to reserve free space on the
    /// hard disk for further partitioning after the installation. Optional,
    /// will be heuristically determined if unset.
    pub hdsize: Option<f64>,
}

#[cfg_attr(feature = "api-types", api(
    properties: {
        hdsize: {
            type: Number,
            minimum: 2.,
            optional: true,
        },
        swapsize: {
            type: Number,
            minimum: 0.,
            optional: true,
        },
        maxroot: {
            type: Number,
            minimum: 2.,
            optional: true,
        },
        maxvz: {
            type: Number,
            minimum: 0.,
            optional: true,
        },
        minfree: {
            type: Number,
            minimum: 0.,
            optional: true,
        },
    },
), derive(Updater))]
#[derive(Clone, Copy, Default, Deserialize, Serialize, Debug, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// LVM-specific filesystem options, when using ext4 or xfs as filesystem.
pub struct LvmOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Size of the root disk to use, can be used to reserve free space on the
    /// hard disk for further partitioning after the installation. Optional,
    /// will be heuristically determined if unset.
    pub hdsize: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Size of the swap volume. Optional, will be heuristically determined if
    /// unset.
    pub swapsize: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Maximum size the `root` volume. Optional, will be heuristically determined
    /// if unset.
    pub maxroot: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Maximum size the `data` volume. Optional, will be heuristically determined
    /// if unset.
    pub maxvz: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Minimum amount of free space that should be left in the LVM volume group.
    /// Optional, will be heuristically determined if unset.
    pub minfree: Option<f64>,
}

#[cfg_attr(feature = "api-types", api(
    properties: {
        hdsize: {
            type: Number,
            minimum: 2.,
            optional: true,
        },
        raid: {
            type: BtrfsRaidLevel,
            optional: true,
        },
        compress: {
            type: BtrfsCompressOption,
            optional: true,
        },
    },
), derive(Updater))]
#[derive(Clone, Copy, Default, Deserialize, Debug, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// Btrfs-specific filesystem options.
pub struct BtrfsOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Size of the root disk. Optional, will be heuristically determined if unset.
    pub hdsize: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// RAID level to use.
    pub raid: Option<BtrfsRaidLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether to enable filesystem-level compression and what type.
    pub compress: Option<BtrfsCompressOption>,
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Copy, Clone, Deserialize, Serialize, Debug, Default, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
/// Keyboard layout of the system.
pub enum KeyboardLayout {
    /// German
    De,
    /// Swiss-German
    DeCh,
    /// Danish
    Dk,
    /// United Kingdom English
    EnGb,
    #[default]
    /// U.S. English
    EnUs,
    /// Spanish
    Es,
    /// Finnish
    Fi,
    /// French
    Fr,
    /// Belgium-French
    FrBe,
    /// Canada-French
    FrCa,
    /// Swiss-French
    FrCh,
    /// Hungarian
    Hu,
    /// Icelandic
    Is,
    /// Italian
    It,
    /// Japanese
    Jp,
    /// Lithuanian
    Lt,
    /// Macedonian
    Mk,
    /// Dutch
    Nl,
    /// Norwegian
    No,
    /// Polish
    Pl,
    /// Portuguese
    Pt,
    /// Brazil-Portuguese
    PtBr,
    /// Swedish
    Se,
    /// Slovenian
    Si,
    /// Turkish
    Tr,
}

impl KeyboardLayout {
    /// Returns the human-readable name for this [`KeyboardLayout`].
    pub fn human_name(&self) -> &str {
        match self {
            Self::Dk => "Danish",
            Self::De => "German",
            Self::DeCh => "Swiss-German",
            Self::EnGb => "United Kingdom",
            Self::EnUs => "U.S. English",
            Self::Es => "Spanish",
            Self::Fi => "Finnish",
            Self::Fr => "French",
            Self::FrBe => "Belgium-French",
            Self::FrCa => "Canada-French",
            Self::FrCh => "Swiss-French",
            Self::Hu => "Hungarian",
            Self::Is => "Icelandic",
            Self::It => "Italian",
            Self::Jp => "Japanese",
            Self::Lt => "Lithuanian",
            Self::Mk => "Macedonian",
            Self::Nl => "Dutch",
            Self::No => "Norwegian",
            Self::Pl => "Polish",
            Self::Pt => "Portuguese",
            Self::PtBr => "Brazil-Portuguese",
            Self::Si => "Slovenian",
            Self::Se => "Swedish",
            Self::Tr => "Turkish",
        }
    }
}

serde_plain::derive_fromstr_from_deserialize!(KeyboardLayout);
serde_plain::derive_display_from_serialize!(KeyboardLayout);

#[cfg_attr(feature = "api-types", api)]
#[derive(Copy, Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
/// Available Btrfs RAID levels.
pub enum BtrfsRaidLevel {
    #[default]
    #[serde(alias = "raid0")]
    /// RAID 0, aka. single or striped.
    Raid0,
    #[serde(alias = "raid1")]
    /// RAID 1, aka. mirror.
    Raid1,
    #[serde(alias = "raid10")]
    /// RAID 10, combining stripe and mirror.
    Raid10,
}

serde_plain::derive_display_from_serialize!(BtrfsRaidLevel);

impl BtrfsRaidLevel {
    /// Minimum number of disks required for this RAID level.
    pub fn min_disks(self) -> usize {
        match self {
            Self::Raid0 => 1,
            Self::Raid1 => 2,
            Self::Raid10 => 4,
        }
    }

    /// Whether this RAID level requires an even number of disks
    /// (e.g. RAID10, which is a stripe of mirrors).
    pub fn requires_even_disks(self) -> bool {
        matches!(self, Self::Raid10)
    }
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Copy, Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
/// Possible compression algorithms usable with Btrfs. See the accompanying
/// mount option in btrfs(5).
pub enum BtrfsCompressOption {
    /// Enable compression, chooses the default algorithm set by Btrfs.
    On,
    #[default]
    /// Disable compression.
    Off,
    /// Use zlib for compression.
    Zlib,
    /// Use zlo for compression.
    Lzo,
    /// Use Zstandard for compression.
    Zstd,
}

serde_plain::derive_display_from_serialize!(BtrfsCompressOption);
serde_plain::derive_fromstr_from_deserialize!(BtrfsCompressOption);

/// List of all available Btrfs compression options.
pub const BTRFS_COMPRESS_OPTIONS: &[BtrfsCompressOption] = {
    use BtrfsCompressOption::*;
    &[On, Off, Zlib, Lzo, Zstd]
};

#[cfg_attr(feature = "api-types", api)]
#[derive(Copy, Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
/// Available ZFS RAID levels.
pub enum ZfsRaidLevel {
    #[default]
    #[serde(alias = "raid0")]
    /// RAID 0, aka. single or striped.
    Raid0,
    #[serde(alias = "raid1")]
    /// RAID 1, aka. mirror.
    Raid1,
    #[serde(alias = "raid10")]
    /// RAID 10, combining stripe and mirror.
    Raid10,
    #[serde(alias = "raidz-1", rename = "RAIDZ-1")]
    /// ZFS-specific RAID level, provides fault tolerance for one disk.
    RaidZ,
    #[serde(alias = "raidz-2", rename = "RAIDZ-2")]
    /// ZFS-specific RAID level, provides fault tolerance for two disks.
    RaidZ2,
    #[serde(alias = "raidz-3", rename = "RAIDZ-3")]
    /// ZFS-specific RAID level, provides fault tolerance for three disks.
    RaidZ3,
}

serde_plain::derive_display_from_serialize!(ZfsRaidLevel);

impl ZfsRaidLevel {
    /// Minimum number of disks required for this RAID level.
    pub fn min_disks(self) -> usize {
        match self {
            Self::Raid0 => 1,
            Self::Raid1 => 2,
            Self::Raid10 => 4,
            Self::RaidZ => 3,
            Self::RaidZ2 => 4,
            Self::RaidZ3 => 5,
        }
    }

    /// Whether this RAID level requires an even number of disks
    /// (e.g. RAID10, which is a stripe of mirrors).
    pub fn requires_even_disks(self) -> bool {
        matches!(self, Self::Raid10)
    }
}

#[cfg_attr(feature = "api-types", api)]
#[derive(Copy, Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
/// Possible compression algorithms usable with ZFS.
pub enum ZfsCompressOption {
    #[default]
    /// Enable compression, chooses the default algorithm set by ZFS.
    On,
    /// Disable compression.
    Off,
    /// Use lzjb for compression.
    Lzjb,
    /// Use lz4 for compression.
    Lz4,
    /// Use zle for compression.
    Zle,
    /// Use gzip for compression.
    Gzip,
    /// Use Zstandard for compression.
    Zstd,
}

serde_plain::derive_display_from_serialize!(ZfsCompressOption);
serde_plain::derive_fromstr_from_deserialize!(ZfsCompressOption);

/// List of all available ZFS compression options.
pub const ZFS_COMPRESS_OPTIONS: &[ZfsCompressOption] = {
    use ZfsCompressOption::*;
    &[On, Off, Lzjb, Lz4, Zle, Gzip, Zstd]
};

#[cfg_attr(feature = "api-types", api)]
#[derive(Copy, Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Possible checksum algorithms usable with ZFS.
pub enum ZfsChecksumOption {
    #[default]
    /// Enable compression, chooses the default algorithm set by ZFS.
    On,
    /// Use Fletcher4 for checksumming.
    Fletcher4,
    /// Use SHA256 for checksumming.
    Sha256,
}

serde_plain::derive_display_from_serialize!(ZfsChecksumOption);
serde_plain::derive_fromstr_from_deserialize!(ZfsChecksumOption);

/// List of all available ZFS checksumming options.
pub const ZFS_CHECKSUM_OPTIONS: &[ZfsChecksumOption] = {
    use ZfsChecksumOption::*;
    &[On, Fletcher4, Sha256]
};

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
/// The filesystem to use for the installation.
pub enum FilesystemType {
    #[default]
    /// Fourth extended filesystem.
    Ext4,
    /// XFS.
    Xfs,
    /// ZFS, with a given RAID level.
    Zfs(ZfsRaidLevel),
    /// Btrfs, with a given RAID level.
    Btrfs(BtrfsRaidLevel),
}

impl FilesystemType {
    /// Returns whether this filesystem is Btrfs.
    pub fn is_btrfs(&self) -> bool {
        matches!(self, FilesystemType::Btrfs(_))
    }

    /// Returns true if the filesystem is used on top of LVM, e.g. ext4 or XFS.
    pub fn is_lvm(&self) -> bool {
        matches!(self, FilesystemType::Ext4 | FilesystemType::Xfs)
    }
}

impl fmt::Display for FilesystemType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Values displayed to the user in the installer UI
        match self {
            FilesystemType::Ext4 => write!(f, "ext4"),
            FilesystemType::Xfs => write!(f, "XFS"),
            FilesystemType::Zfs(level) => write!(f, "ZFS ({level})"),
            FilesystemType::Btrfs(level) => write!(f, "BTRFS ({level})"),
        }
    }
}

impl Serialize for FilesystemType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // These values must match exactly what the low-level installer expects
        let value = match self {
            // proxinstall::$fssetup
            FilesystemType::Ext4 => "ext4",
            FilesystemType::Xfs => "xfs",
            // proxinstall::get_zfs_raid_setup()
            FilesystemType::Zfs(level) => &format!("zfs ({level})"),
            // proxinstall::get_btrfs_raid_setup()
            FilesystemType::Btrfs(level) => &format!("btrfs ({level})"),
        };

        serializer.collect_str(value)
    }
}

impl FromStr for FilesystemType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ext4" => Ok(FilesystemType::Ext4),
            "xfs" => Ok(FilesystemType::Xfs),
            "zfs (RAID0)" => Ok(FilesystemType::Zfs(ZfsRaidLevel::Raid0)),
            "zfs (RAID1)" => Ok(FilesystemType::Zfs(ZfsRaidLevel::Raid1)),
            "zfs (RAID10)" => Ok(FilesystemType::Zfs(ZfsRaidLevel::Raid10)),
            "zfs (RAIDZ-1)" => Ok(FilesystemType::Zfs(ZfsRaidLevel::RaidZ)),
            "zfs (RAIDZ-2)" => Ok(FilesystemType::Zfs(ZfsRaidLevel::RaidZ2)),
            "zfs (RAIDZ-3)" => Ok(FilesystemType::Zfs(ZfsRaidLevel::RaidZ3)),
            "btrfs (RAID0)" => Ok(FilesystemType::Btrfs(BtrfsRaidLevel::Raid0)),
            "btrfs (RAID1)" => Ok(FilesystemType::Btrfs(BtrfsRaidLevel::Raid1)),
            "btrfs (RAID10)" => Ok(FilesystemType::Btrfs(BtrfsRaidLevel::Raid10)),
            _ => Err(format!("Could not find file system: {s}")),
        }
    }
}

serde_plain::derive_deserialize_from_fromstr!(FilesystemType, "valid filesystem");

/// List of all available filesystem types.
pub const FILESYSTEM_TYPE_OPTIONS: &[FilesystemType] = {
    use FilesystemType::*;
    &[
        Ext4,
        Xfs,
        Zfs(ZfsRaidLevel::Raid0),
        Zfs(ZfsRaidLevel::Raid1),
        Zfs(ZfsRaidLevel::Raid10),
        Zfs(ZfsRaidLevel::RaidZ),
        Zfs(ZfsRaidLevel::RaidZ2),
        Zfs(ZfsRaidLevel::RaidZ3),
        Btrfs(BtrfsRaidLevel::Raid0),
        Btrfs(BtrfsRaidLevel::Raid1),
        Btrfs(BtrfsRaidLevel::Raid10),
    ]
};

fn deserialize_non_empty_string_maybe<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val: Option<String> = Deserialize::deserialize(deserializer)?;

    match val {
        Some(s) if !s.is_empty() => Ok(Some(s)),
        _ => Ok(None),
    }
}

/// Validates an email address according to [EMAIL_REGEX]
pub fn email_validate(email: &str) -> Result<()> {
    if !EMAIL_REGEX.is_match(email) {
        bail!("Email does not look like a valid address (user@domain.tld)")
    } else if email == crate::EMAIL_DEFAULT_PLACEHOLDER {
        bail!("Invalid (default) email address")
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disks(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("sd{i}")).collect()
    }

    fn zfs(raid: ZfsRaidLevel, n: usize) -> DiskSetup {
        DiskSetup {
            filesystem: Filesystem::Zfs,
            disk_list: disks(n),
            filter: BTreeMap::new(),
            filter_match: None,
            zfs: Some(ZfsOptions {
                raid: Some(raid),
                ..Default::default()
            }),
            lvm: None,
            btrfs: None,
        }
    }

    fn btrfs(raid: BtrfsRaidLevel, n: usize) -> DiskSetup {
        DiskSetup {
            filesystem: Filesystem::Btrfs,
            disk_list: disks(n),
            filter: BTreeMap::new(),
            filter_match: None,
            zfs: None,
            lvm: None,
            btrfs: Some(BtrfsOptions {
                raid: Some(raid),
                ..Default::default()
            }),
        }
    }

    // Exercises both the min-disks branch and the min_disks() constants
    // themselves through observed behavior, so no separate constant test is
    // needed.
    #[test]
    fn filesystem_details_enforces_min_disks_per_raid_level() {
        use BtrfsRaidLevel as B;
        use ZfsRaidLevel as Z;
        for v in [
            Z::Raid0,
            Z::Raid1,
            Z::Raid10,
            Z::RaidZ,
            Z::RaidZ2,
            Z::RaidZ3,
        ] {
            let min = v.min_disks();
            if min > 1 {
                assert!(zfs(v, min - 1).filesystem_details().is_err(), "ZFS {v}");
            }
            assert!(zfs(v, min).filesystem_details().is_ok(), "ZFS {v}");
        }
        for v in [B::Raid0, B::Raid1, B::Raid10] {
            let min = v.min_disks();
            if min > 1 {
                assert!(btrfs(v, min - 1).filesystem_details().is_err(), "Btrfs {v}");
            }
            assert!(btrfs(v, min).filesystem_details().is_ok(), "Btrfs {v}");
        }
    }

    #[test]
    fn filesystem_details_rejects_raid10_with_odd_disks() {
        // five disks pass the min-disks check (>= 4) and must trip the
        // even-disks check; covers both ZFS and Btrfs RAID10
        assert!(zfs(ZfsRaidLevel::Raid10, 5).filesystem_details().is_err());
        assert!(
            btrfs(BtrfsRaidLevel::Raid10, 5)
                .filesystem_details()
                .is_err()
        );
    }

    #[test]
    fn filesystem_details_skips_disk_count_for_filter_selection() {
        // a filter-based selection resolves disks only at install time, so an
        // empty disk_list must not be rejected even for a RAID level needing
        // many disks
        let mut filter = BTreeMap::new();
        filter.insert("size".to_string(), ">100G".to_string());
        let setup = DiskSetup {
            filesystem: Filesystem::Zfs,
            disk_list: Vec::new(),
            filter,
            filter_match: None,
            zfs: Some(ZfsOptions {
                raid: Some(ZfsRaidLevel::Raid10),
                ..Default::default()
            }),
            lvm: None,
            btrfs: None,
        };
        assert!(setup.filesystem_details().is_ok());
    }

    #[test]
    fn network_bond_requires_at_least_two_members() {
        let mut bond = NetworkBondOptions::default();
        let res = bond.verify();
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "bond setup requires at least 2 interfaces, got 0"
        );

        bond.members = vec!["12:34:56:ab:cd:df".parse().unwrap()];

        let res = bond.verify();
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "bond setup requires at least 2 interfaces, got 1"
        );

        bond.members = vec![
            "12:34:56:ab:cd:df".parse().unwrap(),
            "ab:cd:df:12:34:56".parse().unwrap(),
        ];
        assert!(bond.verify().is_ok());
    }

    #[test]
    fn network_bond_rejects_duplicate_members() {
        let bond = NetworkBondOptions {
            members: vec![
                "12:34:56:ab:cd:df".parse().unwrap(),
                "12:34:56:AB:CD:DF".parse().unwrap(),
            ],
            ..Default::default()
        };

        let res = bond.verify();
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "bond member 12:34:56:AB:CD:DF is given more than once"
        );
    }

    #[test]
    fn network_bond_primary_if_only_with_active_backup() {
        let mut bond = NetworkBondOptions {
            members: vec![
                "12:34:56:ab:cd:df".parse().unwrap(),
                "ab:cd:df:12:34:56".parse().unwrap(),
            ],
            primary_interface: Some("12:34:56:ab:cd:df".parse().unwrap()),
            ..Default::default()
        };

        for m in NetworkBondOptions::MODES {
            bond.mode = *m;
            match m {
                LinuxBondMode::ActiveBackup => assert!(bond.verify().is_ok()),
                _ => {
                    let res = bond.verify();
                    assert!(res.is_err());
                    assert_eq!(
                        res.unwrap_err().to_string(),
                        "primary bond interface can only be set in active-backup mode"
                    );
                }
            }
        }
    }

    #[test]
    fn network_bond_primary_if_must_be_member() {
        let mut bond = NetworkBondOptions {
            members: vec![
                "12:34:56:ab:cd:df".parse().unwrap(),
                "ab:cd:df:12:34:56".parse().unwrap(),
            ],
            primary_interface: Some("12:34:56:ab:cd:df".parse().unwrap()),
            ..Default::default()
        };
        assert!(bond.verify().is_ok());

        bond.primary_interface = Some("ab:12:cd:34:ef:56".parse().unwrap());

        let res = bond.verify();
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "primary bond interface AB:12:CD:34:EF:56 must be a member of the bond (members: 12:34:56:AB:CD:DF, AB:CD:DF:12:34:56)"
        );
    }

    #[test]
    fn network_vlan_id_must_be_valid() {
        let mut vlan = NetworkVlanOptions { vid: 2 };
        assert!(vlan.verify().is_ok());

        vlan.vid = 4094;
        assert!(vlan.verify().is_ok());

        vlan.vid = 1;
        let res = vlan.verify();
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "VLAN tag must be between 2 and 4094 inclusive, got 1"
        );

        vlan.vid = 4095;
        let res = vlan.verify();
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "VLAN tag must be between 2 and 4094 inclusive, got 4095"
        );
    }
}
