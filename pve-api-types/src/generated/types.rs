#[api(
    properties: {
        Arch: {
            type: AptUpdateInfoArch,
        },
        Description: {
            type: String,
        },
        NotifyStatus: {
            optional: true,
            type: String,
        },
        OldVersion: {
            optional: true,
            type: String,
        },
        Origin: {
            type: String,
        },
        Package: {
            type: String,
        },
        Priority: {
            type: String,
        },
        Section: {
            type: String,
        },
        Title: {
            type: String,
        },
        Version: {
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct AptUpdateInfo {
    #[serde(rename = "Arch")]
    pub arch: AptUpdateInfoArch,

    /// Package description.
    #[serde(rename = "Description")]
    pub description: String,

    /// Version for which PVE has already sent an update notification for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "NotifyStatus")]
    pub notify_status: Option<String>,

    /// Old version currently installed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "OldVersion")]
    pub old_version: Option<String>,

    /// Package origin, e.g., 'Proxmox' or 'Debian'.
    #[serde(rename = "Origin")]
    pub origin: String,

    /// Package name.
    #[serde(rename = "Package")]
    pub package: String,

    /// Package priority.
    #[serde(rename = "Priority")]
    pub priority: String,

    /// Package section.
    #[serde(rename = "Section")]
    pub section: String,

    /// Package title.
    #[serde(rename = "Title")]
    pub title: String,

    /// New version to be updated to.
    #[serde(rename = "Version")]
    pub version: String,
}

#[api]
/// Package Architecture.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum AptUpdateInfoArch {
    #[serde(rename = "armhf")]
    /// armhf.
    Armhf,
    #[serde(rename = "arm64")]
    /// arm64.
    Arm64,
    #[serde(rename = "amd64")]
    /// amd64.
    Amd64,
    #[serde(rename = "ppc64el")]
    /// ppc64el.
    Ppc64el,
    #[serde(rename = "risc64")]
    /// risc64.
    Risc64,
    #[serde(rename = "s390x")]
    /// s390x.
    S390x,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(AptUpdateInfoArch);
serde_plain::derive_fromstr_from_deserialize!(AptUpdateInfoArch);

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
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct AptUpdateParams {
    /// Send notification about new packages.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<bool>,

    /// Only produces output suitable for logging, omitting progress indicators.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quiet: Option<bool>,
}

const CLUSTER_RESOURCE_CONTENT: Schema =
    proxmox_schema::ArraySchema::new("list", &StorageContent::API_SCHEMA).schema();

mod cluster_resource_content {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[doc(hidden)]
    pub trait Ser: Sized {
        fn ser<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>;
        fn de<'de, D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>;
    }

    impl<T: Serialize + for<'a> Deserialize<'a>> Ser for Vec<T> {
        fn ser<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            super::stringlist::serialize(&self[..], serializer, &super::CLUSTER_RESOURCE_CONTENT)
        }

        fn de<'de, D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            super::stringlist::deserialize(deserializer, &super::CLUSTER_RESOURCE_CONTENT)
        }
    }

    impl<T: Ser> Ser for Option<T> {
        fn ser<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            match self {
                None => serializer.serialize_none(),
                Some(inner) => inner.ser(serializer),
            }
        }

        fn de<'de, D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            use std::fmt;
            use std::marker::PhantomData;

            struct V<T: Ser>(PhantomData<T>);

            impl<'de, T: Ser> serde::de::Visitor<'de> for V<T> {
                type Value = Option<T>;

                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("an optional string")
                }

                fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                    Ok(None)
                }

                fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    T::de(deserializer).map(Some)
                }

                fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                    use serde::de::IntoDeserializer;
                    T::de(value.into_deserializer()).map(Some)
                }
            }

            deserializer.deserialize_option(V::<T>(PhantomData))
        }
    }

    pub fn serialize<T, S>(this: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
        T: Ser,
    {
        this.ser(serializer)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        D: serde::Deserializer<'de>,
        T: Ser,
    {
        T::de(deserializer)
    }
}

const_regex! {

CLUSTER_JOIN_INFO_PREFERRED_NODE_RE = r##"^(?i:[a-z0-9](?i:[a-z0-9\-]*[a-z0-9])?)$"##;

}

#[test]
fn test_regex_compilation_1() {
    use regex::Regex;
    let _: &Regex = &CLUSTER_JOIN_INFO_PREFERRED_NODE_RE;
}
#[api(
    properties: {
        config_digest: {
            type: String,
            description: "FIXME: Missing description in PVE.",
        },
        nodelist: {
            items: {
                type: ClusterJoinInfoNodelist,
            },
            type: Array,
            description: "FIXME: Missing description in PVE.",
        },
        preferred_node: {
            format: &ApiStringFormat::Pattern(&CLUSTER_JOIN_INFO_PREFERRED_NODE_RE),
            type: String,
        },
        totem: {
            description: "FIXME: missing description in PVE",
            properties: {},
            type: Object,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ClusterJoinInfo {
    pub config_digest: String,

    pub nodelist: Vec<ClusterJoinInfoNodelist>,

    /// The cluster node name.
    pub preferred_node: String,

    pub totem: serde_json::Value,
}

const_regex! {

CLUSTER_JOIN_INFO_NODELIST_NAME_RE = r##"^(?i:[a-z0-9](?i:[a-z0-9\-]*[a-z0-9])?)$"##;

}

#[test]
fn test_regex_compilation_2() {
    use regex::Regex;
    let _: &Regex = &CLUSTER_JOIN_INFO_NODELIST_NAME_RE;
}
#[api(
    additional_properties: "additional_properties",
    properties: {
        name: {
            format: &ApiStringFormat::Pattern(&CLUSTER_JOIN_INFO_NODELIST_NAME_RE),
            type: String,
        },
        nodeid: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        pve_addr: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_ip),
            type: String,
            description: "FIXME: Missing description in PVE.",
        },
        pve_fp: {
            type: String,
            description: "FIXME: Missing description in PVE.",
        },
        quorum_votes: {
            minimum: 0,
            type: Integer,
            description: "FIXME: Missing description in PVE.",
        },
        ring0_addr: {
            format: &ApiStringFormat::PropertyString(&ClusterJoinInfoNodelistRing0Addr::API_SCHEMA),
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ClusterJoinInfoNodelist {
    /// The cluster node name.
    pub name: String,

    /// Node id for this node.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodeid: Option<u64>,

    pub pve_addr: String,

    /// Certificate SHA 256 fingerprint.
    pub pve_fp: String,

    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    pub quorum_votes: u64,

    /// Address and priority information of a single corosync link. (up to 8
    /// links supported; link0..link7)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ring0_addr: Option<String>,

    #[serde(flatten)]
    pub additional_properties: HashMap<String, Value>,
}

#[api(
    default_key: "address",
    properties: {
        address: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_address),
            type: String,
        },
        priority: {
            default: 0,
            maximum: 255,
            minimum: 0,
            optional: true,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ClusterJoinInfoNodelistRing0Addr {
    /// Hostname (or IP) of this corosync link address.
    pub address: String,

    /// The priority for the link when knet is used in 'passive' mode (default).
    /// Lower value means higher priority. Only valid for cluster create,
    /// ignored on node add.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u8")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
}

#[api(
    properties: {
        data: {
            items: {
                type: ClusterMetricsData,
            },
            type: Array,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ClusterMetrics {
    /// Array of system metrics. Metrics are sorted by their timestamp.
    pub data: Vec<ClusterMetricsData>,
}

#[api(
    properties: {
        id: {
            type: String,
        },
        metric: {
            type: String,
        },
        timestamp: {
            type: Integer,
        },
        type: {
            type: ClusterMetricsDataType,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ClusterMetricsData {
    /// Unique identifier for this metric object, for instance 'node/<nodename>'
    /// or 'qemu/<vmid>'.
    pub id: String,

    /// Name of the metric.
    pub metric: String,

    /// Time at which this metric was observed
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub timestamp: i64,

    #[serde(rename = "type")]
    pub ty: ClusterMetricsDataType,

    /// Metric value.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    pub value: f64,
}

#[api]
/// Type of the metric.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum ClusterMetricsDataType {
    #[serde(rename = "gauge")]
    /// gauge.
    Gauge,
    #[serde(rename = "counter")]
    /// counter.
    Counter,
    #[serde(rename = "derive")]
    /// derive.
    Derive,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(ClusterMetricsDataType);
serde_plain::derive_fromstr_from_deserialize!(ClusterMetricsDataType);

const_regex! {

CLUSTER_NODE_INDEX_RESPONSE_NODE_RE = r##"^(?i:[a-z0-9](?i:[a-z0-9\-]*[a-z0-9])?)$"##;

}

#[test]
fn test_regex_compilation_3() {
    use regex::Regex;
    let _: &Regex = &CLUSTER_NODE_INDEX_RESPONSE_NODE_RE;
}
#[api(
    properties: {
        level: {
            optional: true,
            type: String,
        },
        maxcpu: {
            optional: true,
            type: Integer,
        },
        maxmem: {
            optional: true,
            type: Integer,
        },
        mem: {
            optional: true,
            type: Integer,
        },
        node: {
            format: &ApiStringFormat::Pattern(&CLUSTER_NODE_INDEX_RESPONSE_NODE_RE),
            type: String,
        },
        ssl_fingerprint: {
            optional: true,
            type: String,
        },
        status: {
            type: ClusterNodeIndexResponseStatus,
        },
        uptime: {
            optional: true,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ClusterNodeIndexResponse {
    /// CPU utilization.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<f64>,

    /// Support level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,

    /// Number of available CPUs.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxcpu: Option<i64>,

    /// Number of available memory in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxmem: Option<i64>,

    /// Used memory in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mem: Option<i64>,

    /// The cluster node name.
    pub node: String,

    /// The SSL fingerprint for the node certificate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssl_fingerprint: Option<String>,

    pub status: ClusterNodeIndexResponseStatus,

    /// Node uptime in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime: Option<i64>,
}

#[api]
/// Node status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum ClusterNodeIndexResponseStatus {
    #[serde(rename = "unknown")]
    /// unknown.
    Unknown,
    #[serde(rename = "online")]
    /// online.
    Online,
    #[serde(rename = "offline")]
    /// offline.
    Offline,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(ClusterNodeIndexResponseStatus);
serde_plain::derive_fromstr_from_deserialize!(ClusterNodeIndexResponseStatus);

#[api(
    properties: {
        id: {
            type: String,
            description: "FIXME: Missing description in PVE.",
        },
        ip: {
            optional: true,
            type: String,
        },
        level: {
            optional: true,
            type: String,
        },
        local: {
            default: false,
            optional: true,
        },
        name: {
            type: String,
            description: "FIXME: Missing description in PVE.",
        },
        nodeid: {
            optional: true,
            type: Integer,
        },
        nodes: {
            optional: true,
            type: Integer,
        },
        online: {
            default: false,
            optional: true,
        },
        quorate: {
            default: false,
            optional: true,
        },
        type: {
            type: ClusterNodeStatusType,
        },
        version: {
            optional: true,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ClusterNodeStatus {
    pub id: String,

    /// [node] IP of the resolved nodename.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,

    /// [node] Proxmox VE Subscription level, indicates if eligible for
    /// enterprise support as well as access to the stable Proxmox VE Enterprise
    /// Repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,

    /// [node] Indicates if this is the responding node.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<bool>,

    pub name: String,

    /// [node] ID of the node from the corosync configuration.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodeid: Option<i64>,

    /// [cluster] Nodes count, including offline nodes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodes: Option<i64>,

    /// [node] Indicates if the node is online or offline.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub online: Option<bool>,

    /// [cluster] Indicates if there is a majority of nodes online to make
    /// decisions
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quorate: Option<bool>,

    #[serde(rename = "type")]
    pub ty: ClusterNodeStatusType,

    /// [cluster] Current version of the corosync configuration file.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<i64>,
}

#[api]
/// Indicates the type, either cluster or node. The type defines the object
/// properties e.g. quorate available for type cluster.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum ClusterNodeStatusType {
    #[serde(rename = "cluster")]
    /// cluster.
    Cluster,
    #[serde(rename = "node")]
    /// node.
    Node,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(ClusterNodeStatusType);
serde_plain::derive_fromstr_from_deserialize!(ClusterNodeStatusType);

const_regex! {

CLUSTER_RESOURCE_NODE_RE = r##"^(?i:[a-z0-9](?i:[a-z0-9\-]*[a-z0-9])?)$"##;
CLUSTER_RESOURCE_STORAGE_RE = r##"^(?i:[a-z][a-z0-9\-_.]*[a-z0-9])$"##;

}

#[test]
fn test_regex_compilation_4() {
    use regex::Regex;
    let _: &Regex = &CLUSTER_RESOURCE_NODE_RE;
    let _: &Regex = &CLUSTER_RESOURCE_STORAGE_RE;
}
#[api(
    properties: {
        "cgroup-mode": {
            optional: true,
            type: Integer,
        },
        content: {
            format: &ApiStringFormat::PropertyString(&CLUSTER_RESOURCE_CONTENT),
            optional: true,
            type: String,
        },
        cpu: {
            minimum: 0.0,
            optional: true,
        },
        disk: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        diskread: {
            optional: true,
            type: Integer,
        },
        diskwrite: {
            optional: true,
            type: Integer,
        },
        hastate: {
            optional: true,
            type: String,
        },
        id: {
            type: String,
            description: "Resource id.",
        },
        level: {
            optional: true,
            type: String,
        },
        lock: {
            optional: true,
            type: String,
        },
        maxcpu: {
            minimum: 0.0,
            optional: true,
        },
        maxdisk: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        maxmem: {
            optional: true,
            type: Integer,
        },
        mem: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        memhost: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        name: {
            optional: true,
            type: String,
        },
        netin: {
            optional: true,
            type: Integer,
        },
        netout: {
            optional: true,
            type: Integer,
        },
        network: {
            optional: true,
            type: String,
        },
        "network-type": {
            optional: true,
            type: ClusterResourceNetworkType,
        },
        node: {
            format: &ApiStringFormat::Pattern(&CLUSTER_RESOURCE_NODE_RE),
            optional: true,
            type: String,
        },
        plugintype: {
            optional: true,
            type: String,
        },
        pool: {
            optional: true,
            type: String,
        },
        protocol: {
            optional: true,
            type: String,
        },
        sdn: {
            optional: true,
            type: String,
        },
        status: {
            optional: true,
            type: String,
        },
        storage: {
            format: &ApiStringFormat::Pattern(&CLUSTER_RESOURCE_STORAGE_RE),
            optional: true,
            type: String,
        },
        tags: {
            optional: true,
            type: String,
        },
        template: {
            default: false,
            optional: true,
        },
        type: {
            type: ClusterResourceType,
        },
        uptime: {
            optional: true,
            type: Integer,
        },
        vmid: {
            maximum: 999999999,
            minimum: 100,
            optional: true,
            type: Integer,
        },
        "zone-type": {
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ClusterResource {
    /// The cgroup mode the node operates under (for type 'node').
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "cgroup-mode")]
    pub cgroup_mode: Option<i64>,

    /// Allowed storage content types (for type 'storage').
    #[serde(with = "cluster_resource_content")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<StorageContent>>,

    /// CPU utilization (for types 'node', 'qemu' and 'lxc').
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<f64>,

    /// Used disk space in bytes (for type 'storage'), used root image space for
    /// VMs (for types 'qemu' and 'lxc').
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk: Option<u64>,

    /// The number of bytes the guest read from its block devices since the
    /// guest was started. This info is not available for all storage types.
    /// (for types 'qemu' and 'lxc')
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diskread: Option<i64>,

    /// The number of bytes the guest wrote to its block devices since the guest
    /// was started. This info is not available for all storage types. (for
    /// types 'qemu' and 'lxc')
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diskwrite: Option<i64>,

    /// HA service status (for HA managed VMs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hastate: Option<String>,

    /// Resource id.
    pub id: String,

    /// Support level (for type 'node').
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,

    /// The guest's current config lock (for types 'qemu' and 'lxc')
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<String>,

    /// Number of available CPUs (for types 'node', 'qemu' and 'lxc').
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxcpu: Option<f64>,

    /// Storage size in bytes (for type 'storage'), root image size for VMs (for
    /// types 'qemu' and 'lxc').
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxdisk: Option<u64>,

    /// Number of available memory in bytes (for types 'node', 'qemu' and
    /// 'lxc').
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxmem: Option<i64>,

    /// Used memory in bytes (for types 'node', 'qemu' and 'lxc').
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mem: Option<u64>,

    /// Used memory in bytes from the point of view of the host (for types
    /// 'qemu').
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memhost: Option<u64>,

    /// Name of the resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// The amount of traffic in bytes that was sent to the guest over the
    /// network since it was started. (for types 'qemu' and 'lxc')
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netin: Option<i64>,

    /// The amount of traffic in bytes that was sent from the guest over the
    /// network since it was started. (for types 'qemu' and 'lxc')
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netout: Option<i64>,

    /// The name of a Network entity (for type 'network').
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "network-type")]
    pub network_type: Option<ClusterResourceNetworkType>,

    /// The cluster node name (for types 'node', 'storage', 'qemu', and 'lxc').
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,

    /// More specific type, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugintype: Option<String>,

    /// The pool name (for types 'pool', 'qemu' and 'lxc').
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool: Option<String>,

    /// The protocol of a fabric (for type 'network', network-type 'fabric').
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,

    /// The name of an SDN entity (for type 'sdn')
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdn: Option<String>,

    /// Resource type dependent status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    /// The storage identifier (for type 'storage').
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<String>,

    /// The guest's tags (for types 'qemu' and 'lxc')
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,

    /// Determines if the guest is a template. (for types 'qemu' and 'lxc')
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<bool>,

    #[serde(rename = "type")]
    pub ty: ClusterResourceType,

    /// Uptime of node or virtual guest in seconds (for types 'node', 'qemu' and
    /// 'lxc').
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime: Option<i64>,

    /// The numerical vmid (for types 'qemu' and 'lxc').
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vmid: Option<u32>,

    /// The type of an SDN zone (for type 'sdn').
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "zone-type")]
    pub zone_type: Option<String>,
}

#[api]
/// Resource type.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum ClusterResourceKind {
    #[serde(rename = "vm")]
    /// vm.
    Vm,
    #[serde(rename = "storage")]
    /// storage.
    Storage,
    #[serde(rename = "node")]
    /// node.
    Node,
    #[serde(rename = "sdn")]
    /// sdn.
    Sdn,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(ClusterResourceKind);
serde_plain::derive_fromstr_from_deserialize!(ClusterResourceKind);

#[api]
/// The type of network resource (for type 'network').
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum ClusterResourceNetworkType {
    #[serde(rename = "fabric")]
    /// fabric.
    Fabric,
    #[serde(rename = "zone")]
    /// zone.
    Zone,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(ClusterResourceNetworkType);
serde_plain::derive_fromstr_from_deserialize!(ClusterResourceNetworkType);

#[api]
/// Resource type.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum ClusterResourceType {
    #[serde(rename = "node")]
    /// node.
    Node,
    #[serde(rename = "storage")]
    /// storage.
    Storage,
    #[serde(rename = "pool")]
    /// pool.
    Pool,
    #[serde(rename = "qemu")]
    /// qemu.
    Qemu,
    #[serde(rename = "lxc")]
    /// lxc.
    Lxc,
    #[serde(rename = "openvz")]
    /// openvz.
    Openvz,
    #[serde(rename = "sdn")]
    /// sdn.
    Sdn,
    #[serde(rename = "network")]
    /// network.
    Network,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(ClusterResourceType);
serde_plain::derive_fromstr_from_deserialize!(ClusterResourceType);

const_regex! {

CREATE_CONTROLLER_ISIS_IFACES_RE = r##"^[a-zA-Z][a-zA-Z0-9_]{1,20}([:\.]\d+)?$"##;
CREATE_CONTROLLER_ISIS_NET_RE = r##"^[a-fA-F0-9]{2}(\.[a-fA-F0-9]{4}){3,9}\.[a-fA-F0-9]{2}$"##;
CREATE_CONTROLLER_NODE_RE = r##"^(?i:[a-z0-9](?i:[a-z0-9\-]*[a-z0-9])?)$"##;

}

#[test]
fn test_regex_compilation_5() {
    use regex::Regex;
    let _: &Regex = &CREATE_CONTROLLER_ISIS_IFACES_RE;
    let _: &Regex = &CREATE_CONTROLLER_ISIS_NET_RE;
    let _: &Regex = &CREATE_CONTROLLER_NODE_RE;
}
#[api(
    properties: {
        asn: {
            maximum: 4294967295,
            minimum: 0,
            optional: true,
            type: Integer,
        },
        "bgp-multipath-as-path-relax": {
            default: false,
            optional: true,
        },
        controller: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_sdn_controller_id),
            type: String,
        },
        ebgp: {
            default: false,
            optional: true,
        },
        "ebgp-multihop": {
            optional: true,
            type: Integer,
        },
        fabric: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_sdn_id),
            optional: true,
            type: String,
        },
        "isis-domain": {
            optional: true,
            type: String,
        },
        "isis-ifaces": {
            items: {
                description: "List item of type pve-iface.",
                format: &ApiStringFormat::Pattern(&CREATE_CONTROLLER_ISIS_IFACES_RE),
                type: String,
            },
            optional: true,
            type: Array,
        },
        "isis-net": {
            format: &ApiStringFormat::Pattern(&CREATE_CONTROLLER_ISIS_NET_RE),
            optional: true,
            type: String,
        },
        "lock-token": {
            optional: true,
            type: String,
        },
        loopback: {
            optional: true,
            type: String,
        },
        node: {
            format: &ApiStringFormat::Pattern(&CREATE_CONTROLLER_NODE_RE),
            optional: true,
            type: String,
        },
        peers: {
            items: {
                description: "List item of type ip.",
                format: &ApiStringFormat::VerifyFn(verifiers::verify_ip),
                type: String,
            },
            optional: true,
            type: Array,
        },
        type: {
            type: ListControllersType,
        },
    },
)]
/// Object.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct CreateController {
    /// autonomous system number
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asn: Option<u32>,

    /// Consider different AS paths of equal length for multipath computation.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "bgp-multipath-as-path-relax")]
    pub bgp_multipath_as_path_relax: Option<bool>,

    /// The SDN controller object identifier.
    pub controller: String,

    /// Enable eBGP (remote-as external).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ebgp: Option<bool>,

    /// Set maximum amount of hops for eBGP peers.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "ebgp-multihop")]
    pub ebgp_multihop: Option<i64>,

    /// SDN fabric to use as underlay for this EVPN controller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fabric: Option<String>,

    /// Name of the IS-IS domain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "isis-domain")]
    pub isis_domain: Option<String>,

    /// Comma-separated list of interfaces where IS-IS should be active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "isis-ifaces")]
    pub isis_ifaces: Option<Vec<String>>,

    /// Network Entity title for this node in the IS-IS network.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "isis-net")]
    pub isis_net: Option<String>,

    /// the token for unlocking the global SDN configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "lock-token")]
    pub lock_token: Option<String>,

    /// Name of the loopback/dummy interface that provides the Router-IP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loopback: Option<String>,

    /// The cluster node name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,

    /// peers address list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peers: Option<Vec<String>>,

    #[serde(rename = "type")]
    pub ty: ListControllersType,
}

#[api(
    properties: {
        "allow-pending": {
            default: false,
            optional: true,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct CreateSdnLock {
    /// if true, allow acquiring lock even though there are pending changes
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "allow-pending")]
    pub allow_pending: Option<bool>,
}

#[api(
    properties: {
        comment: {
            optional: true,
            type: String,
            description: "Description of the Token",
        },
        expire: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        privsep: {
            default: true,
            optional: true,
        },
    },
)]
/// Object.
#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct CreateToken {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,

    /// API token expiration date (seconds since epoch). '0' means no expiration
    /// date.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expire: Option<u64>,

    /// Restrict API token privileges with separate ACLs (default), or give full
    /// privileges of corresponding user.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privsep: Option<bool>,
}

#[api(
    properties: {
        "full-tokenid": {
            type: String,
        },
        info: {
            type: CreateTokenResponseInfo,
        },
        value: {
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct CreateTokenResponse {
    /// The full token id.
    #[serde(rename = "full-tokenid")]
    pub full_tokenid: String,

    pub info: CreateTokenResponseInfo,

    /// API token value used for authentication.
    pub value: String,
}

#[api(
    properties: {
        comment: {
            optional: true,
            type: String,
            description: "Description of the Token",
        },
        expire: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        privsep: {
            default: true,
            optional: true,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct CreateTokenResponseInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,

    /// API token expiration date (seconds since epoch). '0' means no expiration
    /// date.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expire: Option<u64>,

    /// Restrict API token privileges with separate ACLs (default), or give full
    /// privileges of corresponding user.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privsep: Option<bool>,
}

#[api(
    properties: {
        alias: {
            max_length: 256,
            optional: true,
            type: String,
        },
        "isolate-ports": {
            default: false,
            optional: true,
        },
        "lock-token": {
            optional: true,
            type: String,
        },
        tag: {
            maximum: 16777215,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        type: {
            optional: true,
            type: SdnVnetType,
        },
        vlanaware: {
            default: false,
            optional: true,
        },
        vnet: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_sdn_id),
            type: String,
        },
        zone: {
            type: String,
        },
    },
)]
/// Object.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct CreateVnet {
    /// Alias name of the VNet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,

    /// If true, sets the isolated property for all interfaces on the bridge of
    /// this VNet.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "isolate-ports")]
    pub isolate_ports: Option<bool>,

    /// the token for unlocking the global SDN configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "lock-token")]
    pub lock_token: Option<String>,

    /// VLAN Tag (for VLAN or QinQ zones) or VXLAN VNI (for VXLAN or EVPN
    /// zones).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub ty: Option<SdnVnetType>,

    /// Allow VLANs to pass through this vnet.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlanaware: Option<bool>,

    /// The SDN vnet object identifier.
    pub vnet: String,

    /// Name of the zone this VNet belongs to.
    pub zone: String,
}

const_regex! {

CREATE_ZONE_EXITNODES_RE = r##"^(?i:[a-z0-9](?i:[a-z0-9\-]*[a-z0-9])?)$"##;
CREATE_ZONE_EXITNODES_PRIMARY_RE = r##"^(?i:[a-z0-9](?i:[a-z0-9\-]*[a-z0-9])?)$"##;
CREATE_ZONE_MAC_RE = r##"^(?i)[a-f0-9][02468ace](?::[a-f0-9]{2}){5}$"##;
CREATE_ZONE_NODES_RE = r##"^(?i:[a-z0-9](?i:[a-z0-9\-]*[a-z0-9])?)$"##;

}

#[test]
fn test_regex_compilation_6() {
    use regex::Regex;
    let _: &Regex = &CREATE_ZONE_EXITNODES_RE;
    let _: &Regex = &CREATE_ZONE_EXITNODES_PRIMARY_RE;
    let _: &Regex = &CREATE_ZONE_MAC_RE;
    let _: &Regex = &CREATE_ZONE_NODES_RE;
}
#[api(
    properties: {
        "advertise-subnets": {
            default: false,
            optional: true,
        },
        bridge: {
            optional: true,
            type: String,
        },
        "bridge-disable-mac-learning": {
            default: false,
            optional: true,
        },
        controller: {
            optional: true,
            type: String,
        },
        dhcp: {
            optional: true,
            type: SdnZoneDhcp,
        },
        "disable-arp-nd-suppression": {
            default: false,
            optional: true,
        },
        dns: {
            optional: true,
            type: String,
        },
        dnszone: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_dns_name),
            optional: true,
            type: String,
        },
        "dp-id": {
            optional: true,
            type: Integer,
        },
        exitnodes: {
            items: {
                description: "List item of type pve-node.",
                format: &ApiStringFormat::Pattern(&CREATE_ZONE_EXITNODES_RE),
                type: String,
            },
            optional: true,
            type: Array,
        },
        "exitnodes-local-routing": {
            default: false,
            optional: true,
        },
        "exitnodes-primary": {
            format: &ApiStringFormat::Pattern(&CREATE_ZONE_EXITNODES_PRIMARY_RE),
            optional: true,
            type: String,
        },
        fabric: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_sdn_id),
            optional: true,
            type: String,
        },
        ipam: {
            optional: true,
            type: String,
        },
        "lock-token": {
            optional: true,
            type: String,
        },
        mac: {
            format: &ApiStringFormat::Pattern(&CREATE_ZONE_MAC_RE),
            optional: true,
            type: String,
        },
        mtu: {
            optional: true,
            type: Integer,
        },
        nodes: {
            items: {
                description: "List item of type pve-node.",
                format: &ApiStringFormat::Pattern(&CREATE_ZONE_NODES_RE),
                type: String,
            },
            optional: true,
            type: Array,
        },
        peers: {
            items: {
                description: "List item of type ip.",
                format: &ApiStringFormat::VerifyFn(verifiers::verify_ip),
                type: String,
            },
            optional: true,
            type: Array,
        },
        reversedns: {
            optional: true,
            type: String,
        },
        "rt-import": {
            items: {
                description: "List item of type pve-sdn-bgp-rt.",
                format: &ApiStringFormat::VerifyFn(verifiers::verify_sdn_bgp_rt),
                type: String,
            },
            optional: true,
            type: Array,
        },
        tag: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        type: {
            type: ListZonesType,
        },
        "vlan-protocol": {
            optional: true,
            type: NetworkInterfaceVlanProtocol,
        },
        "vrf-vxlan": {
            maximum: 16777215,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        "vxlan-port": {
            default: 4789,
            maximum: 65536,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        zone: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_sdn_id),
            type: String,
        },
    },
)]
/// Object.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct CreateZone {
    /// Advertise IP prefixes (Type-5 routes) instead of MAC/IP pairs (Type-2
    /// routes).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "advertise-subnets")]
    pub advertise_subnets: Option<bool>,

    /// The bridge for which VLANs should be managed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge: Option<String>,

    /// Disable auto mac learning.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "bridge-disable-mac-learning")]
    pub bridge_disable_mac_learning: Option<bool>,

    /// Controller for this zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controller: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dhcp: Option<SdnZoneDhcp>,

    /// Suppress IPv4 ARP && IPv6 Neighbour Discovery messages.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "disable-arp-nd-suppression")]
    pub disable_arp_nd_suppression: Option<bool>,

    /// dns api server
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns: Option<String>,

    /// dns domain zone  ex: mydomain.com
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dnszone: Option<String>,

    /// Faucet dataplane id
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "dp-id")]
    pub dp_id: Option<i64>,

    /// List of cluster node names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exitnodes: Option<Vec<String>>,

    /// Allow exitnodes to connect to EVPN guests.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "exitnodes-local-routing")]
    pub exitnodes_local_routing: Option<bool>,

    /// Force traffic through this exitnode first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "exitnodes-primary")]
    pub exitnodes_primary: Option<String>,

    /// SDN fabric to use as underlay for this VXLAN zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fabric: Option<String>,

    /// use a specific ipam
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ipam: Option<String>,

    /// the token for unlocking the global SDN configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "lock-token")]
    pub lock_token: Option<String>,

    /// Anycast logical router mac address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,

    /// MTU of the zone, will be used for the created VNet bridges.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtu: Option<i64>,

    /// List of cluster node names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodes: Option<Vec<String>>,

    /// Comma-separated list of peers, that are part of the VXLAN zone. Usually
    /// the IPs of the nodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peers: Option<Vec<String>>,

    /// reverse dns api server
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reversedns: Option<String>,

    /// List of Route Targets that should be imported into the VRF of the zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "rt-import")]
    pub rt_import: Option<Vec<String>>,

    /// Service-VLAN Tag (outer VLAN)
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<u64>,

    #[serde(rename = "type")]
    pub ty: ListZonesType,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vlan-protocol")]
    pub vlan_protocol: Option<NetworkInterfaceVlanProtocol>,

    /// VNI for the zone VRF.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vrf-vxlan")]
    pub vrf_vxlan: Option<u32>,

    /// UDP port that should be used for the VXLAN tunnel (default 4789).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vxlan-port")]
    pub vxlan_port: Option<u32>,

    /// The SDN zone object identifier.
    pub zone: String,
}

#[api]
/// A guest's run state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum IsRunning {
    #[serde(rename = "running")]
    /// running.
    Running,
    #[serde(rename = "stopped")]
    /// stopped.
    Stopped,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(IsRunning);
serde_plain::derive_fromstr_from_deserialize!(IsRunning);

#[api]
/// Only list sdn controllers of specific type
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum ListControllersType {
    #[serde(rename = "bgp")]
    /// bgp.
    Bgp,
    #[serde(rename = "evpn")]
    /// evpn.
    Evpn,
    #[serde(rename = "faucet")]
    /// faucet.
    Faucet,
    #[serde(rename = "isis")]
    /// isis.
    Isis,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(ListControllersType);
serde_plain::derive_fromstr_from_deserialize!(ListControllersType);

#[api]
/// Only list specific interface types.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum ListNetworksType {
    #[serde(rename = "bridge")]
    /// bridge.
    Bridge,
    #[serde(rename = "bond")]
    /// bond.
    Bond,
    #[serde(rename = "eth")]
    /// eth.
    Eth,
    #[serde(rename = "alias")]
    /// alias.
    Alias,
    #[serde(rename = "vlan")]
    /// vlan.
    Vlan,
    #[serde(rename = "fabric")]
    /// fabric.
    Fabric,
    #[serde(rename = "OVSBridge")]
    /// OVSBridge.
    OvsBridge,
    #[serde(rename = "OVSBond")]
    /// OVSBond.
    OvsBond,
    #[serde(rename = "OVSPort")]
    /// OVSPort.
    OvsPort,
    #[serde(rename = "OVSIntPort")]
    /// OVSIntPort.
    OvsIntPort,
    #[serde(rename = "vnet")]
    /// vnet.
    Vnet,
    #[serde(rename = "any_bridge")]
    /// any_bridge.
    AnyBridge,
    #[serde(rename = "any_local_bridge")]
    /// any_local_bridge.
    AnyLocalBridge,
    #[serde(rename = "include_sdn")]
    /// include_sdn.
    IncludeSdn,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(ListNetworksType);
serde_plain::derive_fromstr_from_deserialize!(ListNetworksType);

#[api(
    properties: {
        comment: {
            optional: true,
            type: String,
        },
        realm: {
            type: String,
            description: "FIXME: Missing description in PVE.",
        },
        tfa: {
            optional: true,
            type: ListRealmTfa,
        },
        type: {
            type: String,
            description: "FIXME: Missing description in PVE.",
        },
    },
)]
/// Object.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ListRealm {
    /// A comment. The GUI use this text when you select a domain (Realm) on the
    /// login window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,

    pub realm: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tfa: Option<ListRealmTfa>,

    #[serde(rename = "type")]
    pub ty: String,
}

#[api]
/// Two-factor authentication provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum ListRealmTfa {
    #[serde(rename = "yubico")]
    /// yubico.
    Yubico,
    #[serde(rename = "oath")]
    /// oath.
    Oath,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(ListRealmTfa);
serde_plain::derive_fromstr_from_deserialize!(ListRealmTfa);

const_regex! {

LIST_TASKS_STATUSFILTER_RE = r##"^(?i:ok|error|warning|unknown)$"##;

}

#[test]
fn test_regex_compilation_7() {
    use regex::Regex;
    let _: &Regex = &LIST_TASKS_STATUSFILTER_RE;
}
#[api(
    properties: {
        errors: {
            default: false,
            optional: true,
        },
        limit: {
            default: 50,
            minimum: 0,
            optional: true,
            type: Integer,
        },
        since: {
            optional: true,
            type: Integer,
        },
        source: {
            optional: true,
            type: ListTasksSource,
        },
        start: {
            default: 0,
            minimum: 0,
            optional: true,
            type: Integer,
        },
        statusfilter: {
            items: {
                description: "List item of type pve-task-status-type.",
                format: &ApiStringFormat::Pattern(&LIST_TASKS_STATUSFILTER_RE),
                type: String,
            },
            optional: true,
            type: Array,
        },
        typefilter: {
            optional: true,
            type: String,
        },
        until: {
            optional: true,
            type: Integer,
        },
        userfilter: {
            optional: true,
            type: String,
        },
        vmid: {
            maximum: 999999999,
            minimum: 100,
            optional: true,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct ListTasks {
    /// Only list tasks with a status of ERROR.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub errors: Option<bool>,

    /// Only list this number of tasks.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,

    /// Only list tasks since this UNIX epoch.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ListTasksSource>,

    /// List tasks beginning from this offset.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<u64>,

    /// List of Task States that should be returned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statusfilter: Option<Vec<String>>,

    /// Only list tasks of this type (e.g., vzstart, vzdump).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typefilter: Option<String>,

    /// Only list tasks until this UNIX epoch.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<i64>,

    /// Only list tasks from this user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub userfilter: Option<String>,

    /// Only list tasks for this VM.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vmid: Option<u32>,
}

#[api(
    properties: {
        endtime: {
            optional: true,
            type: Integer,
            description: "The task's end time.",
        },
        id: {
            type: String,
            description: "The task id.",
        },
        node: {
            type: String,
            description: "The task's node.",
        },
        pid: {
            type: Integer,
            description: "The task process id.",
        },
        pstart: {
            type: Integer,
            description: "The task's proc start time.",
        },
        starttime: {
            type: Integer,
            description: "The task's start time.",
        },
        status: {
            optional: true,
            type: String,
            description: "The task's status.",
        },
        type: {
            type: String,
            description: "The task type.",
        },
        upid: {
            type: String,
            description: "The task's UPID.",
        },
        user: {
            type: String,
            description: "The task owner's user id.",
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ListTasksResponse {
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endtime: Option<i64>,

    pub id: String,

    pub node: String,

    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub pid: i64,

    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub pstart: i64,

    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub starttime: i64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    #[serde(rename = "type")]
    pub ty: String,

    pub upid: String,

    pub user: String,
}

#[api]
/// List archived, active or all tasks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum ListTasksSource {
    #[serde(rename = "archive")]
    #[default]
    /// archive.
    Archive,
    #[serde(rename = "active")]
    /// active.
    Active,
    #[serde(rename = "all")]
    /// all.
    All,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(ListTasksSource);
serde_plain::derive_fromstr_from_deserialize!(ListTasksSource);

#[api]
/// Only list SDN zones of specific type
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum ListZonesType {
    #[serde(rename = "evpn")]
    /// evpn.
    Evpn,
    #[serde(rename = "faucet")]
    /// faucet.
    Faucet,
    #[serde(rename = "qinq")]
    /// qinq.
    Qinq,
    #[serde(rename = "simple")]
    /// simple.
    Simple,
    #[serde(rename = "vlan")]
    /// vlan.
    Vlan,
    #[serde(rename = "vxlan")]
    /// vxlan.
    Vxlan,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(ListZonesType);
serde_plain::derive_fromstr_from_deserialize!(ListZonesType);

const_regex! {

LXC_CONFIG_TAGS_RE = r##"^(?i)[a-z0-9_][a-z0-9_\-+.]*$"##;
LXC_CONFIG_TIMEZONE_RE = r##"^.*/.*$"##;

}

#[test]
fn test_regex_compilation_8() {
    use regex::Regex;
    let _: &Regex = &LXC_CONFIG_TAGS_RE;
    let _: &Regex = &LXC_CONFIG_TIMEZONE_RE;
}
#[api(
    properties: {
        arch: {
            optional: true,
            type: LxcConfigArch,
        },
        cmode: {
            optional: true,
            type: LxcConfigCmode,
        },
        console: {
            default: true,
            optional: true,
        },
        cores: {
            maximum: 8192,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        cpulimit: {
            default: 0.0,
            maximum: 8192.0,
            minimum: 0.0,
            optional: true,
        },
        cpuunits: {
            default: 1024,
            maximum: 500000,
            minimum: 0,
            optional: true,
            type: Integer,
        },
        debug: {
            default: false,
            optional: true,
        },
        description: {
            max_length: 8192,
            optional: true,
            type: String,
        },
        dev: {
            type: LxcConfigDevArray,
        },
        digest: {
            type: String,
        },
        features: {
            format: &ApiStringFormat::PropertyString(&LxcConfigFeatures::API_SCHEMA),
            optional: true,
            type: String,
        },
        hookscript: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_volume_id),
            optional: true,
            type: String,
        },
        hostname: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_dns_name),
            max_length: 255,
            optional: true,
            type: String,
        },
        lock: {
            optional: true,
            type: LxcConfigLock,
        },
        lxc: {
            items: {
                items: {
                    type: String,
                    description: "A config key value pair",
                },
                type: Array,
                description: "A raw lxc config entry",
            },
            optional: true,
            type: Array,
        },
        memory: {
            default: 512,
            minimum: 16,
            optional: true,
            type: Integer,
        },
        mp: {
            type: LxcConfigMpArray,
        },
        nameserver: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_ip_with_ll_iface),
            optional: true,
            type: String,
        },
        net: {
            type: LxcConfigNetArray,
        },
        onboot: {
            default: false,
            optional: true,
        },
        ostype: {
            optional: true,
            type: LxcConfigOstype,
        },
        protection: {
            default: false,
            optional: true,
        },
        rootfs: {
            format: &ApiStringFormat::PropertyString(&LxcConfigRootfs::API_SCHEMA),
            optional: true,
            type: String,
        },
        searchdomain: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_dns_name),
            optional: true,
            type: String,
        },
        startup: {
            optional: true,
            type: String,
            type_text: "[[order=]\\d+] [,up=\\d+] [,down=\\d+] ",
        },
        swap: {
            default: 512,
            minimum: 0,
            optional: true,
            type: Integer,
        },
        tags: {
            format: &ApiStringFormat::Pattern(&LXC_CONFIG_TAGS_RE),
            optional: true,
            type: String,
        },
        template: {
            default: false,
            optional: true,
        },
        timezone: {
            format: &ApiStringFormat::Pattern(&LXC_CONFIG_TIMEZONE_RE),
            optional: true,
            type: String,
        },
        tty: {
            default: 2,
            maximum: 6,
            minimum: 0,
            optional: true,
            type: Integer,
        },
        unprivileged: {
            default: false,
            optional: true,
        },
        unused: {
            type: LxcConfigUnusedArray,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LxcConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<LxcConfigArch>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmode: Option<LxcConfigCmode>,

    /// Attach a console device (/dev/console) to the container.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub console: Option<bool>,

    /// The number of cores assigned to the container. A container can use all
    /// available cores by default.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u16")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cores: Option<u16>,

    /// Limit of CPU usage.
    ///
    /// NOTE: If the computer has 2 CPUs, it has a total of '2' CPU time. Value
    /// '0' indicates no CPU limit.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpulimit: Option<f64>,

    /// CPU weight for a container, will be clamped to [1, 10000] in cgroup v2.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpuunits: Option<u32>,

    /// Try to be more verbose. For now this only enables debug log-level on
    /// start.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<bool>,

    /// Description for the Container. Shown in the web-interface CT's summary.
    /// This is saved as comment inside the configuration file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Device to pass through to the container
    #[serde(flatten)]
    pub dev: LxcConfigDevArray,

    /// SHA1 digest of configuration file. This can be used to prevent
    /// concurrent modifications.
    pub digest: String,

    /// Allow containers access to advanced features.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub features: Option<String>,

    /// Script that will be executed during various steps in the containers
    /// lifetime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hookscript: Option<String>,

    /// Set a host name for the container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<LxcConfigLock>,

    /// Array of lxc low-level configurations ([[key1, value1], [key2, value2]
    /// ...]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lxc: Option<Vec<Vec<String>>>,

    /// Amount of RAM for the container in MB.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<u64>,

    /// Use volume as container mount point. Use the special syntax
    /// STORAGE_ID:SIZE_IN_GiB to allocate a new volume.
    #[serde(flatten)]
    pub mp: LxcConfigMpArray,

    /// Sets DNS server IP address for a container. Create will automatically
    /// use the setting from the host if you neither set searchdomain nor
    /// nameserver.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nameserver: Option<String>,

    /// Specifies network interfaces for the container.
    #[serde(flatten)]
    pub net: LxcConfigNetArray,

    /// Specifies whether a container will be started during system bootup.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onboot: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ostype: Option<LxcConfigOstype>,

    /// Sets the protection flag of the container. This will prevent the CT or
    /// CT's disk remove/update operation.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protection: Option<bool>,

    /// Use volume as container root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rootfs: Option<String>,

    /// Sets DNS search domains for a container. Create will automatically use
    /// the setting from the host if you neither set searchdomain nor
    /// nameserver.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub searchdomain: Option<String>,

    /// Startup and shutdown behavior. Order is a non-negative number defining
    /// the general startup order. Shutdown in done with reverse ordering.
    /// Additionally you can set the 'up' or 'down' delay in seconds, which
    /// specifies a delay to wait before the next VM is started or stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startup: Option<String>,

    /// Amount of SWAP for the container in MB.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap: Option<u64>,

    /// Tags of the Container. This is only meta information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,

    /// Enable/disable Template.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<bool>,

    /// Time zone to use in the container. If option isn't set, then nothing
    /// will be done. Can be set to 'host' to match the host time zone, or an
    /// arbitrary time zone option from /usr/share/zoneinfo/zone.tab
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,

    /// Specify the number of tty available to the container
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u8")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tty: Option<u8>,

    /// Makes the container run as unprivileged user. For creation, the default
    /// is 1. For restore, the default is the value from the backup. (Should not
    /// be modified manually.)
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unprivileged: Option<bool>,

    /// Reference to unused volumes. This is used internally, and should not be
    /// modified manually.
    #[serde(flatten)]
    pub unused: LxcConfigUnusedArray,
}
generate_array_field! {
    LxcConfigDevArray [ 256 ] :
    r#"Device to pass through to the container"#,
    String => {
        description: "Device to pass through to the container",
        format: &ApiStringFormat::PropertyString(&LxcConfigDev::API_SCHEMA),
        type: String,
    }
    dev
}
generate_array_field! {
    LxcConfigMpArray [ 256 ] :
    r#"Use volume as container mount point. Use the special syntax STORAGE_ID:SIZE_IN_GiB to allocate a new volume."#,
    String => {
        description: "Use volume as container mount point. Use the special syntax STORAGE_ID:SIZE_IN_GiB to allocate a new volume.",
        format: &ApiStringFormat::PropertyString(&LxcConfigMp::API_SCHEMA),
        type: String,
    }
    mp
}
generate_array_field! {
    LxcConfigNetArray [ 32 ] :
    r#"Specifies network interfaces for the container."#,
    String => {
        description: "Specifies network interfaces for the container.",
        format: &ApiStringFormat::PropertyString(&LxcConfigNet::API_SCHEMA),
        type: String,
    }
    net
}
generate_array_field! {
    LxcConfigUnusedArray [ 256 ] :
    r#"Reference to unused volumes. This is used internally, and should not be modified manually."#,
    String => {
        description: "Reference to unused volumes. This is used internally, and should not be modified manually.",
        format: &ApiStringFormat::PropertyString(&LxcConfigUnused::API_SCHEMA),
        type: String,
    }
    unused
}

#[api]
/// OS architecture type.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum LxcConfigArch {
    #[serde(rename = "amd64")]
    #[default]
    /// amd64.
    Amd64,
    #[serde(rename = "i386")]
    /// i386.
    I386,
    #[serde(rename = "arm64")]
    /// arm64.
    Arm64,
    #[serde(rename = "armhf")]
    /// armhf.
    Armhf,
    #[serde(rename = "riscv32")]
    /// riscv32.
    Riscv32,
    #[serde(rename = "riscv64")]
    /// riscv64.
    Riscv64,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(LxcConfigArch);
serde_plain::derive_fromstr_from_deserialize!(LxcConfigArch);

#[api]
/// Console mode. By default, the console command tries to open a connection to
/// one of the available tty devices. By setting cmode to 'console' it tries to
/// attach to /dev/console instead. If you set cmode to 'shell', it simply
/// invokes a shell inside the container (no login).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum LxcConfigCmode {
    #[serde(rename = "shell")]
    /// shell.
    Shell,
    #[serde(rename = "console")]
    /// console.
    Console,
    #[serde(rename = "tty")]
    #[default]
    /// tty.
    Tty,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(LxcConfigCmode);
serde_plain::derive_fromstr_from_deserialize!(LxcConfigCmode);

#[api(
    default_key: "path",
    properties: {
        "deny-write": {
            default: false,
            optional: true,
        },
        gid: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        mode: {
            optional: true,
            type: String,
        },
        path: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_lxc_dev_string),
            optional: true,
            type: String,
        },
        uid: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LxcConfigDev {
    /// Deny the container to write to the device
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "deny-write")]
    pub deny_write: Option<bool>,

    /// Group ID to be assigned to the device node
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gid: Option<u64>,

    /// Access mode to be set on the device node
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,

    /// Device to pass through to the container
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// User ID to be assigned to the device node
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<u64>,
}

#[api(
    properties: {
        force_rw_sys: {
            default: false,
            optional: true,
        },
        fuse: {
            default: false,
            optional: true,
        },
        keyctl: {
            default: false,
            optional: true,
        },
        mknod: {
            default: false,
            optional: true,
        },
        mount: {
            optional: true,
            type: String,
        },
        nesting: {
            default: false,
            optional: true,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LxcConfigFeatures {
    /// Mount /sys in unprivileged containers as `rw` instead of `mixed`. This
    /// can break networking under newer (>= v245) systemd-network use.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_rw_sys: Option<bool>,

    /// Allow using 'fuse' file systems in a container. Note that interactions
    /// between fuse and the freezer cgroup can potentially cause I/O deadlocks.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuse: Option<bool>,

    /// For unprivileged containers only: Allow the use of the keyctl() system
    /// call. This is required to use docker inside a container. By default
    /// unprivileged containers will see this system call as non-existent. This
    /// is mostly a workaround for systemd-networkd, as it will treat it as a
    /// fatal error when some keyctl() operations are denied by the kernel due
    /// to lacking permissions. Essentially, you can choose between running
    /// systemd-networkd or docker.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyctl: Option<bool>,

    /// Allow unprivileged containers to use mknod() to add certain device
    /// nodes. This requires a kernel with seccomp trap to user space support
    /// (5.3 or newer). This is experimental.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mknod: Option<bool>,

    /// Allow mounting file systems of specific types. This should be a list of
    /// file system types as used with the mount command. Note that this can
    /// have negative effects on the container's security. With access to a loop
    /// device, mounting a file can circumvent the mknod permission of the
    /// devices cgroup, mounting an NFS file system can block the host's I/O
    /// completely and prevent it from rebooting, etc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mount: Option<String>,

    /// Allow nesting. Best used with unprivileged containers with additional id
    /// mapping. Note that this will expose procfs and sysfs contents of the
    /// host to the guest.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nesting: Option<bool>,
}

#[api]
/// Lock/unlock the container.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum LxcConfigLock {
    #[serde(rename = "backup")]
    /// backup.
    Backup,
    #[serde(rename = "create")]
    /// create.
    Create,
    #[serde(rename = "destroyed")]
    /// destroyed.
    Destroyed,
    #[serde(rename = "disk")]
    /// disk.
    Disk,
    #[serde(rename = "fstrim")]
    /// fstrim.
    Fstrim,
    #[serde(rename = "migrate")]
    /// migrate.
    Migrate,
    #[serde(rename = "mounted")]
    /// mounted.
    Mounted,
    #[serde(rename = "rollback")]
    /// rollback.
    Rollback,
    #[serde(rename = "snapshot")]
    /// snapshot.
    Snapshot,
    #[serde(rename = "snapshot-delete")]
    /// snapshot-delete.
    SnapshotDelete,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(LxcConfigLock);
serde_plain::derive_fromstr_from_deserialize!(LxcConfigLock);

const_regex! {

LXC_CONFIG_MP_SIZE_RE = r##"^(\d+(\.\d+)?)([KMGT])?$"##;

}

#[test]
fn test_regex_compilation_9() {
    use regex::Regex;
    let _: &Regex = &LXC_CONFIG_MP_SIZE_RE;
}
#[api(
    default_key: "volume",
    properties: {
        acl: {
            default: false,
            optional: true,
        },
        backup: {
            default: false,
            optional: true,
        },
        mountoptions: {
            optional: true,
            type: String,
        },
        mp: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_lxc_mp_string),
            type: String,
        },
        quota: {
            default: false,
            optional: true,
        },
        replicate: {
            default: true,
            optional: true,
        },
        ro: {
            default: false,
            optional: true,
        },
        shared: {
            default: false,
            optional: true,
        },
        size: {
            format: &ApiStringFormat::Pattern(&LXC_CONFIG_MP_SIZE_RE),
            optional: true,
            type: String,
        },
        volume: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_lxc_mp_string),
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LxcConfigMp {
    /// Explicitly enable or disable ACL support.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acl: Option<bool>,

    /// Whether to include the mount point in backups.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<bool>,

    /// Extra mount options for rootfs/mps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mountoptions: Option<String>,

    /// Path to the mount point as seen from inside the container (must not
    /// contain symlinks).
    pub mp: String,

    /// Enable user quotas inside the container (not supported with zfs
    /// subvolumes)
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota: Option<bool>,

    /// Will include this volume to a storage replica job.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicate: Option<bool>,

    /// Read-only mount point
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ro: Option<bool>,

    /// Mark this non-volume mount point as available on multiple nodes (see
    /// 'nodes')
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared: Option<bool>,

    /// Volume size (read only value).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    /// Volume, device or directory to mount into the container.
    pub volume: String,
}

const_regex! {

LXC_CONFIG_NET_HWADDR_RE = r##"^(?i)[a-f0-9][02468ace](?::[a-f0-9]{2}){5}$"##;

}

#[test]
fn test_regex_compilation_10() {
    use regex::Regex;
    let _: &Regex = &LXC_CONFIG_NET_HWADDR_RE;
}
#[api(
    properties: {
        bridge: {
            optional: true,
            type: String,
        },
        firewall: {
            default: false,
            optional: true,
        },
        gw: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_ipv4),
            optional: true,
            type: String,
        },
        gw6: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_ipv6),
            optional: true,
            type: String,
        },
        hwaddr: {
            format: &ApiStringFormat::Pattern(&LXC_CONFIG_NET_HWADDR_RE),
            optional: true,
            type: String,
        },
        ip: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_ipv4_config),
            optional: true,
            type: String,
        },
        ip6: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_ipv6_config),
            optional: true,
            type: String,
        },
        link_down: {
            default: false,
            optional: true,
        },
        mtu: {
            maximum: 65535,
            minimum: 64,
            optional: true,
            type: Integer,
        },
        name: {
            type: String,
        },
        tag: {
            maximum: 4094,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        trunks: {
            optional: true,
            type: String,
        },
        type: {
            optional: true,
            type: LxcConfigNetType,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LxcConfigNet {
    /// Bridge to attach the network device to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge: Option<String>,

    /// Controls whether this interface's firewall rules should be used.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firewall: Option<bool>,

    /// Default gateway for IPv4 traffic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gw: Option<String>,

    /// Default gateway for IPv6 traffic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gw6: Option<String>,

    /// The interface MAC address. This is dynamically allocated by default, but
    /// you can set that statically if needed, for example to always have the
    /// same link-local IPv6 address. (lxc.network.hwaddr)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hwaddr: Option<String>,

    /// IPv4 address in CIDR format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,

    /// IPv6 address in CIDR format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip6: Option<String>,

    /// Whether this interface should be disconnected (like pulling the plug).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_down: Option<bool>,

    /// Maximum transfer unit of the interface. (lxc.network.mtu)
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u16")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtu: Option<u16>,

    /// Name of the network device as seen from inside the container.
    /// (lxc.network.name)
    pub name: String,

    /// Apply rate limiting to the interface
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<f64>,

    /// VLAN tag for this interface.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u16")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<u16>,

    /// VLAN ids to pass through the interface
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trunks: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub ty: Option<LxcConfigNetType>,
}

#[api]
/// Network interface type.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum LxcConfigNetType {
    #[serde(rename = "veth")]
    /// veth.
    Veth,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(LxcConfigNetType);
serde_plain::derive_fromstr_from_deserialize!(LxcConfigNetType);

#[api]
/// OS type. This is used to setup configuration inside the container, and
/// corresponds to lxc setup scripts in
/// /usr/share/lxc/config/<ostype>.common.conf. Value 'unmanaged' can be used to
/// skip and OS specific setup.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum LxcConfigOstype {
    #[serde(rename = "debian")]
    /// debian.
    Debian,
    #[serde(rename = "devuan")]
    /// devuan.
    Devuan,
    #[serde(rename = "ubuntu")]
    /// ubuntu.
    Ubuntu,
    #[serde(rename = "centos")]
    /// centos.
    Centos,
    #[serde(rename = "fedora")]
    /// fedora.
    Fedora,
    #[serde(rename = "opensuse")]
    /// opensuse.
    Opensuse,
    #[serde(rename = "archlinux")]
    /// archlinux.
    Archlinux,
    #[serde(rename = "alpine")]
    /// alpine.
    Alpine,
    #[serde(rename = "gentoo")]
    /// gentoo.
    Gentoo,
    #[serde(rename = "nixos")]
    /// nixos.
    Nixos,
    #[serde(rename = "unmanaged")]
    /// unmanaged.
    Unmanaged,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(LxcConfigOstype);
serde_plain::derive_fromstr_from_deserialize!(LxcConfigOstype);

const_regex! {

LXC_CONFIG_ROOTFS_SIZE_RE = r##"^(\d+(\.\d+)?)([KMGT])?$"##;

}

#[test]
fn test_regex_compilation_11() {
    use regex::Regex;
    let _: &Regex = &LXC_CONFIG_ROOTFS_SIZE_RE;
}
#[api(
    default_key: "volume",
    properties: {
        acl: {
            default: false,
            optional: true,
        },
        mountoptions: {
            optional: true,
            type: String,
        },
        quota: {
            default: false,
            optional: true,
        },
        replicate: {
            default: true,
            optional: true,
        },
        ro: {
            default: false,
            optional: true,
        },
        shared: {
            default: false,
            optional: true,
        },
        size: {
            format: &ApiStringFormat::Pattern(&LXC_CONFIG_ROOTFS_SIZE_RE),
            optional: true,
            type: String,
        },
        volume: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_lxc_mp_string),
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LxcConfigRootfs {
    /// Explicitly enable or disable ACL support.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acl: Option<bool>,

    /// Extra mount options for rootfs/mps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mountoptions: Option<String>,

    /// Enable user quotas inside the container (not supported with zfs
    /// subvolumes)
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota: Option<bool>,

    /// Will include this volume to a storage replica job.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicate: Option<bool>,

    /// Read-only mount point
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ro: Option<bool>,

    /// Mark this non-volume mount point as available on multiple nodes (see
    /// 'nodes')
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared: Option<bool>,

    /// Volume size (read only value).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    /// Volume, device or directory to mount into the container.
    pub volume: String,
}

#[api(
    default_key: "volume",
    properties: {
        volume: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_volume_id),
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LxcConfigUnused {
    /// The volume that is not used currently.
    pub volume: String,
}

#[api(
    properties: {
        disk: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        diskread: {
            optional: true,
            type: Integer,
        },
        diskwrite: {
            optional: true,
            type: Integer,
        },
        lock: {
            optional: true,
            type: String,
        },
        maxdisk: {
            optional: true,
            type: Integer,
        },
        maxmem: {
            optional: true,
            type: Integer,
        },
        maxswap: {
            optional: true,
            type: Integer,
        },
        mem: {
            optional: true,
            type: Integer,
        },
        name: {
            optional: true,
            type: String,
        },
        netin: {
            optional: true,
            type: Integer,
        },
        netout: {
            optional: true,
            type: Integer,
        },
        status: {
            type: IsRunning,
        },
        tags: {
            optional: true,
            type: String,
        },
        template: {
            default: false,
            optional: true,
        },
        uptime: {
            optional: true,
            type: Integer,
        },
        vmid: {
            maximum: 999999999,
            minimum: 100,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LxcEntry {
    /// Current CPU usage.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<f64>,

    /// Maximum usable CPUs.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpus: Option<f64>,

    /// Root disk image space-usage in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk: Option<u64>,

    /// The amount of bytes the guest read from it's block devices since the
    /// guest was started. (Note: This info is not available for all storage
    /// types.)
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diskread: Option<i64>,

    /// The amount of bytes the guest wrote from it's block devices since the
    /// guest was started. (Note: This info is not available for all storage
    /// types.)
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diskwrite: Option<i64>,

    /// The current config lock, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<String>,

    /// Root disk image size in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxdisk: Option<i64>,

    /// Maximum memory in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxmem: Option<i64>,

    /// Maximum SWAP memory in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxswap: Option<i64>,

    /// Currently used memory in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mem: Option<i64>,

    /// Container name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// The amount of traffic in bytes that was sent to the guest over the
    /// network since it was started.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netin: Option<i64>,

    /// The amount of traffic in bytes that was sent from the guest over the
    /// network since it was started.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netout: Option<i64>,

    /// CPU Some pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressurecpusome: Option<f64>,

    /// IO Full pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressureiofull: Option<f64>,

    /// IO Some pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressureiosome: Option<f64>,

    /// Memory Full pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressurememoryfull: Option<f64>,

    /// Memory Some pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressurememorysome: Option<f64>,

    pub status: IsRunning,

    /// The current configured tags, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,

    /// Determines if the guest is a template.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<bool>,

    /// Uptime in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime: Option<i64>,

    /// The (unique) ID of the VM.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    pub vmid: u32,
}

#[api(
    properties: {
        disk: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        diskread: {
            optional: true,
            type: Integer,
        },
        diskwrite: {
            optional: true,
            type: Integer,
        },
        ha: {
            description: "HA manager service status.",
            properties: {},
            type: Object,
        },
        lock: {
            optional: true,
            type: String,
        },
        maxdisk: {
            optional: true,
            type: Integer,
        },
        maxmem: {
            optional: true,
            type: Integer,
        },
        maxswap: {
            optional: true,
            type: Integer,
        },
        mem: {
            optional: true,
            type: Integer,
        },
        name: {
            optional: true,
            type: String,
        },
        netin: {
            optional: true,
            type: Integer,
        },
        netout: {
            optional: true,
            type: Integer,
        },
        status: {
            type: IsRunning,
        },
        tags: {
            optional: true,
            type: String,
        },
        template: {
            default: false,
            optional: true,
        },
        uptime: {
            optional: true,
            type: Integer,
        },
        vmid: {
            maximum: 999999999,
            minimum: 100,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LxcStatus {
    /// Current CPU usage.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<f64>,

    /// Maximum usable CPUs.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpus: Option<f64>,

    /// Root disk image space-usage in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk: Option<u64>,

    /// The amount of bytes the guest read from it's block devices since the
    /// guest was started. (Note: This info is not available for all storage
    /// types.)
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diskread: Option<i64>,

    /// The amount of bytes the guest wrote from it's block devices since the
    /// guest was started. (Note: This info is not available for all storage
    /// types.)
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diskwrite: Option<i64>,

    /// HA manager service status.
    pub ha: serde_json::Value,

    /// The current config lock, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<String>,

    /// Root disk image size in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxdisk: Option<i64>,

    /// Maximum memory in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxmem: Option<i64>,

    /// Maximum SWAP memory in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxswap: Option<i64>,

    /// Currently used memory in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mem: Option<i64>,

    /// Container name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// The amount of traffic in bytes that was sent to the guest over the
    /// network since it was started.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netin: Option<i64>,

    /// The amount of traffic in bytes that was sent from the guest over the
    /// network since it was started.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netout: Option<i64>,

    /// CPU Some pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressurecpusome: Option<f64>,

    /// IO Full pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressureiofull: Option<f64>,

    /// IO Some pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressureiosome: Option<f64>,

    /// Memory Full pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressurememoryfull: Option<f64>,

    /// Memory Some pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressurememorysome: Option<f64>,

    pub status: IsRunning,

    /// The current configured tags, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,

    /// Determines if the guest is a template.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<bool>,

    /// Uptime in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime: Option<i64>,

    /// The (unique) ID of the VM.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    pub vmid: u32,
}

const_regex! {

MIGRATE_LXC_TARGET_RE = r##"^(?i:[a-z0-9](?i:[a-z0-9\-]*[a-z0-9])?)$"##;
MIGRATE_LXC_TARGET_STORAGE_RE = r##"^(?i:[a-z][a-z0-9\-_.]*[a-z0-9]):(?i:[a-z][a-z0-9\-_.]*[a-z0-9])|(?i:[a-z][a-z0-9\-_.]*[a-z0-9])|1$"##;

}

#[test]
fn test_regex_compilation_12() {
    use regex::Regex;
    let _: &Regex = &MIGRATE_LXC_TARGET_RE;
    let _: &Regex = &MIGRATE_LXC_TARGET_STORAGE_RE;
}
#[api(
    properties: {
        bwlimit: {
            minimum: 0.0,
            optional: true,
        },
        online: {
            default: false,
            optional: true,
        },
        restart: {
            default: false,
            optional: true,
        },
        target: {
            format: &ApiStringFormat::Pattern(&MIGRATE_LXC_TARGET_RE),
            type: String,
        },
        "target-storage": {
            items: {
                description: "List item of type storage-pair.",
                format: &ApiStringFormat::Pattern(&MIGRATE_LXC_TARGET_STORAGE_RE),
                type: String,
            },
            optional: true,
            type: Array,
        },
        timeout: {
            default: 180,
            optional: true,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct MigrateLxc {
    /// Override I/O bandwidth limit (in KiB/s).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bwlimit: Option<f64>,

    /// Use online/live migration.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub online: Option<bool>,

    /// Use restart migration
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart: Option<bool>,

    /// Target node.
    pub target: String,

    /// Mapping from source to target storages. Providing only a single storage
    /// ID maps all source storages to that storage. Providing the special value
    /// '1' will map each source storage to itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "target-storage")]
    pub target_storage: Option<Vec<String>>,

    /// Timeout in seconds for shutdown for restart migration
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<i64>,
}

const_regex! {

MIGRATE_QEMU_TARGET_RE = r##"^(?i:[a-z0-9](?i:[a-z0-9\-]*[a-z0-9])?)$"##;
MIGRATE_QEMU_TARGETSTORAGE_RE = r##"^(?i:[a-z][a-z0-9\-_.]*[a-z0-9]):(?i:[a-z][a-z0-9\-_.]*[a-z0-9])|(?i:[a-z][a-z0-9\-_.]*[a-z0-9])|1$"##;

}

#[test]
fn test_regex_compilation_13() {
    use regex::Regex;
    let _: &Regex = &MIGRATE_QEMU_TARGET_RE;
    let _: &Regex = &MIGRATE_QEMU_TARGETSTORAGE_RE;
}
#[api(
    properties: {
        bwlimit: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        force: {
            default: false,
            optional: true,
        },
        migration_network: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_cidr),
            optional: true,
            type: String,
        },
        migration_type: {
            optional: true,
            type: StartQemuMigrationType,
        },
        online: {
            default: false,
            optional: true,
        },
        target: {
            format: &ApiStringFormat::Pattern(&MIGRATE_QEMU_TARGET_RE),
            type: String,
        },
        targetstorage: {
            items: {
                description: "List item of type storage-pair.",
                format: &ApiStringFormat::Pattern(&MIGRATE_QEMU_TARGETSTORAGE_RE),
                type: String,
            },
            optional: true,
            type: Array,
        },
        "with-conntrack-state": {
            default: false,
            optional: true,
        },
        "with-local-disks": {
            default: false,
            optional: true,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct MigrateQemu {
    /// Override I/O bandwidth limit (in KiB/s).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bwlimit: Option<u64>,

    /// Allow to migrate VMs which use local devices. Only root may use this
    /// option.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,

    /// CIDR of the (sub) network that is used for migration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_network: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_type: Option<StartQemuMigrationType>,

    /// Use online/live migration if VM is running. Ignored if VM is stopped.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub online: Option<bool>,

    /// Target node.
    pub target: String,

    /// Mapping from source to target storages. Providing only a single storage
    /// ID maps all source storages to that storage. Providing the special value
    /// '1' will map each source storage to itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targetstorage: Option<Vec<String>>,

    /// Whether to migrate conntrack entries for running VMs.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "with-conntrack-state")]
    pub with_conntrack_state: Option<bool>,

    /// Enable live storage migration for local disk
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "with-local-disks")]
    pub with_local_disks: Option<bool>,
}

const_regex! {

NETWORK_INTERFACE_BOND_PRIMARY_RE = r##"^[a-zA-Z][a-zA-Z0-9_]{1,20}([:\.]\d+)?$"##;
NETWORK_INTERFACE_BRIDGE_PORTS_RE = r##"^[a-zA-Z][a-zA-Z0-9_]{1,20}([:\.]\d+)?$"##;
NETWORK_INTERFACE_IFACE_RE = r##"^[a-zA-Z][a-zA-Z0-9_]{1,20}([:\.]\d+)?$"##;
NETWORK_INTERFACE_OVS_BONDS_RE = r##"^[a-zA-Z][a-zA-Z0-9_]{1,20}([:\.]\d+)?$"##;
NETWORK_INTERFACE_OVS_BRIDGE_RE = r##"^[a-zA-Z][a-zA-Z0-9_]{1,20}([:\.]\d+)?$"##;
NETWORK_INTERFACE_OVS_PORTS_RE = r##"^[a-zA-Z][a-zA-Z0-9_]{1,20}([:\.]\d+)?$"##;
NETWORK_INTERFACE_SLAVES_RE = r##"^[a-zA-Z][a-zA-Z0-9_]{1,20}([:\.]\d+)?$"##;
NETWORK_INTERFACE_VLAN_RAW_DEVICE_RE = r##"^[a-zA-Z][a-zA-Z0-9_]{1,20}([:\.]\d+)?$"##;

}

#[test]
fn test_regex_compilation_14() {
    use regex::Regex;
    let _: &Regex = &NETWORK_INTERFACE_BOND_PRIMARY_RE;
    let _: &Regex = &NETWORK_INTERFACE_BRIDGE_PORTS_RE;
    let _: &Regex = &NETWORK_INTERFACE_IFACE_RE;
    let _: &Regex = &NETWORK_INTERFACE_OVS_BONDS_RE;
    let _: &Regex = &NETWORK_INTERFACE_OVS_BRIDGE_RE;
    let _: &Regex = &NETWORK_INTERFACE_OVS_PORTS_RE;
    let _: &Regex = &NETWORK_INTERFACE_SLAVES_RE;
    let _: &Regex = &NETWORK_INTERFACE_VLAN_RAW_DEVICE_RE;
}
#[api(
    properties: {
        active: {
            default: false,
            optional: true,
        },
        address: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_ipv4),
            optional: true,
            type: String,
        },
        address6: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_ipv6),
            optional: true,
            type: String,
        },
        autostart: {
            default: false,
            optional: true,
        },
        "bond-primary": {
            format: &ApiStringFormat::Pattern(&NETWORK_INTERFACE_BOND_PRIMARY_RE),
            optional: true,
            type: String,
        },
        bond_mode: {
            optional: true,
            type: NetworkInterfaceBondMode,
        },
        bond_xmit_hash_policy: {
            optional: true,
            type: NetworkInterfaceBondXmitHashPolicy,
        },
        "bridge-access": {
            optional: true,
            type: Integer,
        },
        "bridge-arp-nd-suppress": {
            default: false,
            optional: true,
        },
        "bridge-learning": {
            default: false,
            optional: true,
        },
        "bridge-multicast-flood": {
            default: false,
            optional: true,
        },
        "bridge-unicast-flood": {
            default: false,
            optional: true,
        },
        bridge_ports: {
            format: &ApiStringFormat::Pattern(&NETWORK_INTERFACE_BRIDGE_PORTS_RE),
            optional: true,
            type: String,
        },
        bridge_vids: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_vlan_id_or_range),
            optional: true,
            type: String,
        },
        bridge_vlan_aware: {
            default: false,
            optional: true,
        },
        cidr: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_cidrv4),
            optional: true,
            type: String,
        },
        cidr6: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_cidrv6),
            optional: true,
            type: String,
        },
        comments: {
            optional: true,
            type: String,
        },
        comments6: {
            optional: true,
            type: String,
        },
        exists: {
            default: false,
            optional: true,
        },
        families: {
            items: {
                type: NetworkInterfaceFamilies,
            },
            optional: true,
            type: Array,
        },
        gateway: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_ipv4),
            optional: true,
            type: String,
        },
        gateway6: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_ipv6),
            optional: true,
            type: String,
        },
        iface: {
            format: &ApiStringFormat::Pattern(&NETWORK_INTERFACE_IFACE_RE),
            max_length: 20,
            min_length: 2,
            type: String,
        },
        "link-type": {
            optional: true,
            type: String,
        },
        method: {
            optional: true,
            type: NetworkInterfaceMethod,
        },
        method6: {
            optional: true,
            type: NetworkInterfaceMethod,
        },
        mtu: {
            maximum: 65520,
            minimum: 1280,
            optional: true,
            type: Integer,
        },
        netmask: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_ipv4_mask),
            optional: true,
            type: String,
        },
        netmask6: {
            maximum: 128,
            minimum: 0,
            optional: true,
            type: Integer,
        },
        options: {
            items: {
                description: "An interface property.",
                type: String,
            },
            optional: true,
            type: Array,
        },
        options6: {
            items: {
                description: "An interface property.",
                type: String,
            },
            optional: true,
            type: Array,
        },
        ovs_bonds: {
            format: &ApiStringFormat::Pattern(&NETWORK_INTERFACE_OVS_BONDS_RE),
            optional: true,
            type: String,
        },
        ovs_bridge: {
            format: &ApiStringFormat::Pattern(&NETWORK_INTERFACE_OVS_BRIDGE_RE),
            optional: true,
            type: String,
        },
        ovs_options: {
            max_length: 1024,
            optional: true,
            type: String,
        },
        ovs_ports: {
            format: &ApiStringFormat::Pattern(&NETWORK_INTERFACE_OVS_PORTS_RE),
            optional: true,
            type: String,
        },
        ovs_tag: {
            maximum: 4094,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        priority: {
            optional: true,
            type: Integer,
        },
        slaves: {
            format: &ApiStringFormat::Pattern(&NETWORK_INTERFACE_SLAVES_RE),
            optional: true,
            type: String,
        },
        type: {
            type: NetworkInterfaceType,
        },
        "uplink-id": {
            optional: true,
            type: String,
        },
        "vlan-id": {
            maximum: 4094,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        "vlan-protocol": {
            optional: true,
            type: NetworkInterfaceVlanProtocol,
        },
        "vlan-raw-device": {
            format: &ApiStringFormat::Pattern(&NETWORK_INTERFACE_VLAN_RAW_DEVICE_RE),
            optional: true,
            type: String,
        },
        "vxlan-id": {
            optional: true,
            type: Integer,
        },
        "vxlan-local-tunnelip": {
            optional: true,
            type: String,
        },
        "vxlan-physdev": {
            optional: true,
            type: String,
        },
        "vxlan-svcnodeip": {
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct NetworkInterface {
    /// Set to true if the interface is active.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,

    /// IP address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,

    /// IP address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address6: Option<String>,

    /// Automatically start interface on boot.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autostart: Option<bool>,

    /// Specify the primary interface for active-backup bond.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "bond-primary")]
    pub bond_primary: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bond_mode: Option<NetworkInterfaceBondMode>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bond_xmit_hash_policy: Option<NetworkInterfaceBondXmitHashPolicy>,

    /// The bridge port access VLAN.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "bridge-access")]
    pub bridge_access: Option<i64>,

    /// Bridge port ARP/ND suppress flag.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "bridge-arp-nd-suppress")]
    pub bridge_arp_nd_suppress: Option<bool>,

    /// Bridge port learning flag.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "bridge-learning")]
    pub bridge_learning: Option<bool>,

    /// Bridge port multicast flood flag.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "bridge-multicast-flood")]
    pub bridge_multicast_flood: Option<bool>,

    /// Bridge port unicast flood flag.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "bridge-unicast-flood")]
    pub bridge_unicast_flood: Option<bool>,

    /// Specify the interfaces you want to add to your bridge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_ports: Option<String>,

    /// Specify the allowed VLANs. For example: '2 4 100-200'. Only used if the
    /// bridge is VLAN aware.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_vids: Option<String>,

    /// Enable bridge vlan support.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_vlan_aware: Option<bool>,

    /// IPv4 CIDR.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cidr: Option<String>,

    /// IPv6 CIDR.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cidr6: Option<String>,

    /// Comments
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comments: Option<String>,

    /// Comments
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comments6: Option<String>,

    /// Set to true if the interface physically exists.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exists: Option<bool>,

    /// The network families.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub families: Option<Vec<NetworkInterfaceFamilies>>,

    /// Default gateway address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,

    /// Default ipv6 gateway address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway6: Option<String>,

    /// Network interface name.
    pub iface: String,

    /// The link type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "link-type")]
    pub link_type: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<NetworkInterfaceMethod>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method6: Option<NetworkInterfaceMethod>,

    /// MTU.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u16")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtu: Option<u16>,

    /// Network mask.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netmask: Option<String>,

    /// Network mask.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u8")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netmask6: Option<u8>,

    /// A list of additional interface options for IPv4.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,

    /// A list of additional interface options for IPv6.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options6: Option<Vec<String>>,

    /// Specify the interfaces used by the bonding device.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ovs_bonds: Option<String>,

    /// The OVS bridge associated with a OVS port. This is required when you
    /// create an OVS port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ovs_bridge: Option<String>,

    /// OVS interface options.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ovs_options: Option<String>,

    /// Specify the interfaces you want to add to your bridge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ovs_ports: Option<String>,

    /// Specify a VLan tag (used by OVSPort, OVSIntPort, OVSBond)
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u16")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ovs_tag: Option<u16>,

    /// The order of the interface.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i64>,

    /// Specify the interfaces used by the bonding device.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slaves: Option<String>,

    #[serde(rename = "type")]
    pub ty: NetworkInterfaceType,

    /// The uplink ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "uplink-id")]
    pub uplink_id: Option<String>,

    /// vlan-id for a custom named vlan interface (ifupdown2 only).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u16")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vlan-id")]
    pub vlan_id: Option<u16>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vlan-protocol")]
    pub vlan_protocol: Option<NetworkInterfaceVlanProtocol>,

    /// Specify the raw interface for the vlan interface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vlan-raw-device")]
    pub vlan_raw_device: Option<String>,

    /// The VXLAN ID.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vxlan-id")]
    pub vxlan_id: Option<i64>,

    /// The VXLAN local tunnel IP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vxlan-local-tunnelip")]
    pub vxlan_local_tunnelip: Option<String>,

    /// The physical device for the VXLAN tunnel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vxlan-physdev")]
    pub vxlan_physdev: Option<String>,

    /// The VXLAN SVC node IP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vxlan-svcnodeip")]
    pub vxlan_svcnodeip: Option<String>,
}

#[api]
/// Bonding mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum NetworkInterfaceBondMode {
    #[serde(rename = "balance-rr")]
    /// balance-rr.
    BalanceRr,
    #[serde(rename = "active-backup")]
    /// active-backup.
    ActiveBackup,
    #[serde(rename = "balance-xor")]
    /// balance-xor.
    BalanceXor,
    #[serde(rename = "broadcast")]
    /// broadcast.
    Broadcast,
    #[serde(rename = "802.3ad")]
    /// 802.3ad.
    Ieee802_3ad,
    #[serde(rename = "balance-tlb")]
    /// balance-tlb.
    BalanceTlb,
    #[serde(rename = "balance-alb")]
    /// balance-alb.
    BalanceAlb,
    #[serde(rename = "balance-slb")]
    /// balance-slb.
    BalanceSlb,
    #[serde(rename = "lacp-balance-slb")]
    /// lacp-balance-slb.
    LacpBalanceSlb,
    #[serde(rename = "lacp-balance-tcp")]
    /// lacp-balance-tcp.
    LacpBalanceTcp,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(NetworkInterfaceBondMode);
serde_plain::derive_fromstr_from_deserialize!(NetworkInterfaceBondMode);

#[api]
/// Selects the transmit hash policy to use for slave selection in balance-xor
/// and 802.3ad modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum NetworkInterfaceBondXmitHashPolicy {
    #[serde(rename = "layer2")]
    /// layer2.
    Layer2,
    #[serde(rename = "layer2+3")]
    /// layer2+3.
    Layer2_3,
    #[serde(rename = "layer3+4")]
    /// layer3+4.
    Layer3_4,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(NetworkInterfaceBondXmitHashPolicy);
serde_plain::derive_fromstr_from_deserialize!(NetworkInterfaceBondXmitHashPolicy);

#[api]
/// A network family.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum NetworkInterfaceFamilies {
    #[serde(rename = "inet")]
    /// inet.
    Inet,
    #[serde(rename = "inet6")]
    /// inet6.
    Inet6,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(NetworkInterfaceFamilies);
serde_plain::derive_fromstr_from_deserialize!(NetworkInterfaceFamilies);

#[api]
/// The network configuration method for IPv4.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum NetworkInterfaceMethod {
    #[serde(rename = "loopback")]
    /// loopback.
    Loopback,
    #[serde(rename = "dhcp")]
    /// dhcp.
    Dhcp,
    #[serde(rename = "manual")]
    /// manual.
    Manual,
    #[serde(rename = "static")]
    /// static.
    Static,
    #[serde(rename = "auto")]
    /// auto.
    Auto,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(NetworkInterfaceMethod);
serde_plain::derive_fromstr_from_deserialize!(NetworkInterfaceMethod);

#[api]
/// Network interface type
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum NetworkInterfaceType {
    #[serde(rename = "bridge")]
    /// bridge.
    Bridge,
    #[serde(rename = "bond")]
    /// bond.
    Bond,
    #[serde(rename = "eth")]
    /// eth.
    Eth,
    #[serde(rename = "alias")]
    /// alias.
    Alias,
    #[serde(rename = "vlan")]
    /// vlan.
    Vlan,
    #[serde(rename = "fabric")]
    /// fabric.
    Fabric,
    #[serde(rename = "OVSBridge")]
    /// OVSBridge.
    Ovsbridge,
    #[serde(rename = "OVSBond")]
    /// OVSBond.
    Ovsbond,
    #[serde(rename = "OVSPort")]
    /// OVSPort.
    Ovsport,
    #[serde(rename = "OVSIntPort")]
    /// OVSIntPort.
    OvsintPort,
    #[serde(rename = "vnet")]
    /// vnet.
    Vnet,
    #[serde(rename = "unknown")]
    /// unknown.
    Unknown,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(NetworkInterfaceType);
serde_plain::derive_fromstr_from_deserialize!(NetworkInterfaceType);

#[api]
/// The VLAN protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum NetworkInterfaceVlanProtocol {
    #[serde(rename = "802.1ad")]
    /// 802.1ad.
    Ieee802_1ad,
    #[serde(rename = "802.1q")]
    /// 802.1q.
    Ieee802_1q,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(NetworkInterfaceVlanProtocol);
serde_plain::derive_fromstr_from_deserialize!(NetworkInterfaceVlanProtocol);

#[api(
    properties: {
        cmd: {
            optional: true,
            type: NodeShellTermproxyCmd,
        },
        "cmd-opts": {
            default: "",
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct NodeShellTermproxy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmd: Option<NodeShellTermproxyCmd>,

    /// Add parameters to a command. Encoded as null terminated strings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "cmd-opts")]
    pub cmd_opts: Option<String>,
}

#[api]
/// Run specific command or default to login (requires 'root@pam')
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum NodeShellTermproxyCmd {
    #[serde(rename = "ceph_install")]
    /// ceph_install.
    CephInstall,
    #[serde(rename = "login")]
    #[default]
    /// login.
    Login,
    #[serde(rename = "upgrade")]
    /// upgrade.
    Upgrade,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(NodeShellTermproxyCmd);
serde_plain::derive_fromstr_from_deserialize!(NodeShellTermproxyCmd);

#[api(
    properties: {
        port: {
            type: Integer,
        },
        ticket: {
            type: String,
        },
        upid: {
            type: String,
        },
        user: {
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct NodeShellTicket {
    /// port used to bind termproxy to
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub port: i64,

    /// ticket used to verifiy websocket connection
    pub ticket: String,

    /// UPID for termproxy worker task
    pub upid: String,

    /// user
    pub user: String,
}

#[api(
    additional_properties: "additional_properties",
    properties: {
        "boot-info": {
            type: NodeStatusBootInfo,
        },
        cpuinfo: {
            type: NodeStatusCpuinfo,
        },
        "current-kernel": {
            type: NodeStatusCurrentKernel,
        },
        loadavg: {
            items: {
                description: "The value of the load.",
                type: String,
            },
            type: Array,
        },
        memory: {
            type: NodeStatusMemory,
        },
        pveversion: {
            type: String,
        },
        rootfs: {
            type: NodeStatusRootfs,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct NodeStatus {
    /// Meta-information about the boot mode.
    #[serde(rename = "boot-info")]
    pub boot_info: NodeStatusBootInfo,

    /// The current cpu usage.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    pub cpu: f64,

    pub cpuinfo: NodeStatusCpuinfo,

    /// Meta-information about the currently booted kernel of this node.
    #[serde(rename = "current-kernel")]
    pub current_kernel: NodeStatusCurrentKernel,

    /// An array of load avg for 1, 5 and 15 minutes respectively.
    pub loadavg: Vec<String>,

    pub memory: NodeStatusMemory,

    /// The PVE version string.
    pub pveversion: String,

    pub rootfs: NodeStatusRootfs,

    #[serde(flatten)]
    pub additional_properties: HashMap<String, Value>,
}

#[api(
    properties: {
        mode: {
            type: NodeStatusBootInfoMode,
        },
        secureboot: {
            default: false,
            optional: true,
        },
    },
)]
/// Meta-information about the boot mode.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct NodeStatusBootInfo {
    pub mode: NodeStatusBootInfoMode,

    /// System is booted in secure mode, only applicable for the "efi" mode.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secureboot: Option<bool>,
}

#[api]
/// Through which firmware the system got booted.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum NodeStatusBootInfoMode {
    #[serde(rename = "efi")]
    /// efi.
    Efi,
    #[serde(rename = "legacy-bios")]
    /// legacy-bios.
    LegacyBios,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(NodeStatusBootInfoMode);
serde_plain::derive_fromstr_from_deserialize!(NodeStatusBootInfoMode);

#[api(
    properties: {
        cores: {
            type: Integer,
        },
        cpus: {
            type: Integer,
        },
        model: {
            type: String,
        },
        sockets: {
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct NodeStatusCpuinfo {
    /// The number of physical cores of the CPU.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub cores: i64,

    /// The number of logical threads of the CPU.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub cpus: i64,

    /// The CPU model
    pub model: String,

    /// The number of logical threads of the CPU.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub sockets: i64,
}

#[api(
    properties: {
        machine: {
            type: String,
        },
        release: {
            type: String,
        },
        sysname: {
            type: String,
        },
        version: {
            type: String,
        },
    },
)]
/// Meta-information about the currently booted kernel of this node.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct NodeStatusCurrentKernel {
    /// Hardware (architecture) type
    pub machine: String,

    /// OS kernel release (e.g., "6.8.0")
    pub release: String,

    /// OS kernel name (e.g., "Linux")
    pub sysname: String,

    /// OS kernel version with build info
    pub version: String,
}

#[api(
    properties: {
        available: {
            optional: true,
            type: Integer,
        },
        free: {
            type: Integer,
        },
        total: {
            type: Integer,
        },
        used: {
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct NodeStatusMemory {
    /// The available memory in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available: Option<i64>,

    /// The free memory in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub free: i64,

    /// The total memory in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub total: i64,

    /// The used memory in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub used: i64,
}

#[api(
    properties: {
        avail: {
            type: Integer,
        },
        free: {
            type: Integer,
        },
        total: {
            type: Integer,
        },
        used: {
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct NodeStatusRootfs {
    /// The available bytes in the root filesystem.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub avail: i64,

    /// The free bytes on the root filesystem.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub free: i64,

    /// The total size of the root filesystem in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub total: i64,

    /// The used bytes in the root filesystem.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub used: i64,
}

#[api(
    properties: {
        checktime: {
            optional: true,
            type: Integer,
        },
        key: {
            optional: true,
            type: String,
        },
        level: {
            optional: true,
            type: String,
        },
        message: {
            optional: true,
            type: String,
        },
        nextduedate: {
            optional: true,
            type: String,
        },
        productname: {
            optional: true,
            type: String,
        },
        regdate: {
            optional: true,
            type: String,
        },
        serverid: {
            optional: true,
            type: String,
        },
        signature: {
            optional: true,
            type: String,
        },
        sockets: {
            optional: true,
            type: Integer,
        },
        status: {
            type: NodeSubscriptionInfoStatus,
        },
        url: {
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct NodeSubscriptionInfo {
    /// Timestamp of the last check done.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checktime: Option<i64>,

    /// The subscription key, if set and permitted to access.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    /// A short code for the subscription level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,

    /// A more human readable status message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Next due date of the set subscription.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nextduedate: Option<String>,

    /// Human readable productname of the set subscription.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub productname: Option<String>,

    /// Register date of the set subscription.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regdate: Option<String>,

    /// The server ID, if permitted to access.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serverid: Option<String>,

    /// Signature for offline keys
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,

    /// The number of sockets for this host.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sockets: Option<i64>,

    pub status: NodeSubscriptionInfoStatus,

    /// URL to the web shop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[api]
/// The current subscription status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum NodeSubscriptionInfoStatus {
    #[serde(rename = "new")]
    /// new.
    New,
    #[serde(rename = "notfound")]
    /// notfound.
    NotFound,
    #[serde(rename = "active")]
    /// active.
    Active,
    #[serde(rename = "invalid")]
    /// invalid.
    Invalid,
    #[serde(rename = "expired")]
    /// expired.
    Expired,
    #[serde(rename = "suspended")]
    /// suspended.
    Suspended,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(NodeSubscriptionInfoStatus);
serde_plain::derive_fromstr_from_deserialize!(NodeSubscriptionInfoStatus);

#[api(
    properties: {
        apitoken: {
            type: String,
        },
        fingerprint: {
            optional: true,
            type: String,
        },
        host: {
            type: String,
        },
        port: {
            optional: true,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ProxmoxRemote {
    /// A full Proxmox API token including the secret value.
    pub apitoken: String,

    /// Remote host's certificate fingerprint, if not trusted by system store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,

    /// Remote Proxmox hostname or IP
    pub host: String,

    /// Port to connect to
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<i64>,
}

#[api(
    default_key: "type",
    properties: {
        "allow-smt": {
            default: true,
            optional: true,
        },
        "kernel-hashes": {
            default: false,
            optional: true,
        },
        "no-debug": {
            default: false,
            optional: true,
        },
        "no-key-sharing": {
            default: false,
            optional: true,
        },
        type: {
            type: PveQemuSevFmtType,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PveQemuSevFmt {
    /// Sets policy bit to allow Simultaneous Multi Threading (SMT) (Ignored
    /// unless for SEV-SNP)
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "allow-smt")]
    pub allow_smt: Option<bool>,

    /// Add kernel hashes to guest firmware for measured linux kernel launch
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "kernel-hashes")]
    pub kernel_hashes: Option<bool>,

    /// Sets policy bit to disallow debugging of guest
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "no-debug")]
    pub no_debug: Option<bool>,

    /// Sets policy bit to disallow key sharing with other guests (Ignored for
    /// SEV-SNP)
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "no-key-sharing")]
    pub no_key_sharing: Option<bool>,

    #[serde(rename = "type")]
    pub ty: PveQemuSevFmtType,
}

#[api]
/// Enable standard SEV with type='std' or enable experimental SEV-ES with the
/// 'es' option or enable experimental SEV-SNP with the 'snp' option.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum PveQemuSevFmtType {
    #[serde(rename = "std")]
    /// std.
    Std,
    #[serde(rename = "es")]
    /// es.
    Es,
    #[serde(rename = "snp")]
    /// snp.
    Snp,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(PveQemuSevFmtType);
serde_plain::derive_fromstr_from_deserialize!(PveQemuSevFmtType);

#[api(
    default_key: "legacy",
    properties: {
        legacy: {
            default: "cdn",
            optional: true,
            type: String,
        },
        order: {
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PveQmBoot {
    /// Boot on floppy (a), hard disk (c), CD-ROM (d), or network (n).
    /// Deprecated, use 'order=' instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy: Option<String>,

    /// The guest will attempt to boot from devices in the order they appear
    /// here.
    ///
    /// Disks, optical drives and passed-through storage USB devices will be
    /// directly booted from, NICs will load PXE, and PCIe devices will
    /// either behave like disks (e.g. NVMe) or load an option ROM (e.g.
    /// RAID controller, hardware NIC).
    ///
    /// Note that only devices in this list will be marked as bootable and thus
    /// loaded by the guest firmware (BIOS/UEFI). If you require multiple
    /// disks for booting (e.g. software-raid), you need to specify all of
    /// them here.
    ///
    /// Overrides the deprecated 'legacy=[acdn]*' value when given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
}

#[api(
    properties: {
        meta: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_volume_id),
            optional: true,
            type: String,
        },
        network: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_volume_id),
            optional: true,
            type: String,
        },
        user: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_volume_id),
            optional: true,
            type: String,
        },
        vendor: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_volume_id),
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PveQmCicustom {
    /// Specify a custom file containing all meta data passed to the VM via
    /// cloud-init. This is provider specific meaning configdrive2 and nocloud
    /// differ.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<String>,

    /// To pass a custom file containing all network data to the VM via
    /// cloud-init.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,

    /// To pass a custom file containing all user data to the VM via cloud-init.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// To pass a custom file containing all vendor data to the VM via
    /// cloud-init.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
}

const_regex! {

PVE_QM_HOSTPCI_MAPPING_RE = r##"^(?i:[a-z][a-z0-9_-]+)$"##;

}

#[test]
fn test_regex_compilation_15() {
    use regex::Regex;
    let _: &Regex = &PVE_QM_HOSTPCI_MAPPING_RE;
}
#[api(
    default_key: "host",
    properties: {
        "device-id": {
            optional: true,
            type: String,
        },
        host: {
            optional: true,
            type: String,
        },
        "legacy-igd": {
            default: false,
            optional: true,
        },
        mapping: {
            format: &ApiStringFormat::Pattern(&PVE_QM_HOSTPCI_MAPPING_RE),
            optional: true,
            type: String,
        },
        mdev: {
            optional: true,
            type: String,
        },
        pcie: {
            default: false,
            optional: true,
        },
        rombar: {
            default: true,
            optional: true,
        },
        romfile: {
            optional: true,
            type: String,
        },
        "sub-device-id": {
            optional: true,
            type: String,
        },
        "sub-vendor-id": {
            optional: true,
            type: String,
        },
        "vendor-id": {
            optional: true,
            type: String,
        },
        "x-vga": {
            default: false,
            optional: true,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PveQmHostpci {
    /// Override PCI device ID visible to guest
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "device-id")]
    pub device_id: Option<String>,

    /// Host PCI device pass through. The PCI ID of a host's PCI device or a
    /// list of PCI virtual functions of the host. HOSTPCIID syntax is:
    ///
    /// 'bus:dev.func' (hexadecimal numbers)
    ///
    /// You can use the 'lspci' command to list existing PCI devices.
    ///
    /// Either this or the 'mapping' key must be set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,

    /// Pass this device in legacy IGD mode, making it the primary and exclusive
    /// graphics device in the VM. Requires 'pc-i440fx' machine type and VGA set
    /// to 'none'.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "legacy-igd")]
    pub legacy_igd: Option<bool>,

    /// The ID of a cluster wide mapping. Either this or the default-key 'host'
    /// must be set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mapping: Option<String>,

    /// The type of mediated device to use.
    /// An instance of this type will be created on startup of the VM and
    /// will be cleaned up when the VM stops.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mdev: Option<String>,

    /// Choose the PCI-express bus (needs the 'q35' machine model).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pcie: Option<bool>,

    /// Specify whether or not the device's ROM will be visible in the guest's
    /// memory map.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rombar: Option<bool>,

    /// Custom pci device rom filename (must be located in /usr/share/kvm/).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub romfile: Option<String>,

    /// Override PCI subsystem device ID visible to guest
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "sub-device-id")]
    pub sub_device_id: Option<String>,

    /// Override PCI subsystem vendor ID visible to guest
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "sub-vendor-id")]
    pub sub_vendor_id: Option<String>,

    /// Override PCI vendor ID visible to guest
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vendor-id")]
    pub vendor_id: Option<String>,

    /// Enable vfio-vga device support.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "x-vga")]
    pub x_vga: Option<bool>,
}

const_regex! {

PVE_QM_IDE_MODEL_RE = r##"^[-%a-zA-Z0-9_.!~*'()]*$"##;
PVE_QM_IDE_SERIAL_RE = r##"^[-%a-zA-Z0-9_.!~*'()]*$"##;
PVE_QM_IDE_SIZE_RE = r##"^(\d+(\.\d+)?)([KMGT])?$"##;

}

#[test]
fn test_regex_compilation_16() {
    use regex::Regex;
    let _: &Regex = &PVE_QM_IDE_MODEL_RE;
    let _: &Regex = &PVE_QM_IDE_SERIAL_RE;
    let _: &Regex = &PVE_QM_IDE_SIZE_RE;
}
#[api(
    default_key: "file",
    properties: {
        aio: {
            optional: true,
            type: PveQmIdeAio,
        },
        backup: {
            default: false,
            optional: true,
        },
        bps: {
            optional: true,
            type: Integer,
        },
        bps_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        bps_rd: {
            optional: true,
            type: Integer,
        },
        bps_rd_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        bps_wr: {
            optional: true,
            type: Integer,
        },
        bps_wr_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        cache: {
            optional: true,
            type: PveQmIdeCache,
        },
        detect_zeroes: {
            default: false,
            optional: true,
        },
        discard: {
            optional: true,
            type: PveQmIdeDiscard,
        },
        file: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_volume_id_or_qm_path),
            type: String,
        },
        format: {
            optional: true,
            type: PveQmIdeFormat,
        },
        iops: {
            optional: true,
            type: Integer,
        },
        iops_max: {
            optional: true,
            type: Integer,
        },
        iops_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iops_rd: {
            optional: true,
            type: Integer,
        },
        iops_rd_max: {
            optional: true,
            type: Integer,
        },
        iops_rd_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iops_wr: {
            optional: true,
            type: Integer,
        },
        iops_wr_max: {
            optional: true,
            type: Integer,
        },
        iops_wr_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        media: {
            optional: true,
            type: PveQmIdeMedia,
        },
        model: {
            format: &ApiStringFormat::Pattern(&PVE_QM_IDE_MODEL_RE),
            max_length: 120,
            optional: true,
            type: String,
        },
        replicate: {
            default: true,
            optional: true,
        },
        rerror: {
            optional: true,
            type: PveQmIdeRerror,
        },
        serial: {
            format: &ApiStringFormat::Pattern(&PVE_QM_IDE_SERIAL_RE),
            max_length: 60,
            optional: true,
            type: String,
        },
        shared: {
            default: false,
            optional: true,
        },
        size: {
            format: &ApiStringFormat::Pattern(&PVE_QM_IDE_SIZE_RE),
            optional: true,
            type: String,
        },
        snapshot: {
            default: false,
            optional: true,
        },
        ssd: {
            default: false,
            optional: true,
        },
        werror: {
            optional: true,
            type: PveQmIdeWerror,
        },
        wwn: {
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PveQmIde {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aio: Option<PveQmIdeAio>,

    /// Whether the drive should be included when making backups.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<bool>,

    /// Maximum r/w speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps: Option<i64>,

    /// Maximum length of I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_max_length: Option<u64>,

    /// Maximum read speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_rd: Option<i64>,

    /// Maximum length of read I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_rd_max_length: Option<u64>,

    /// Maximum write speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_wr: Option<i64>,

    /// Maximum length of write I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_wr_max_length: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<PveQmIdeCache>,

    /// Controls whether to detect and try to optimize writes of zeroes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect_zeroes: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discard: Option<PveQmIdeDiscard>,

    /// The drive's backing volume.
    pub file: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<PveQmIdeFormat>,

    /// Maximum r/w I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops: Option<i64>,

    /// Maximum unthrottled r/w I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_max: Option<i64>,

    /// Maximum length of I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_max_length: Option<u64>,

    /// Maximum read I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd: Option<i64>,

    /// Maximum unthrottled read I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd_max: Option<i64>,

    /// Maximum length of read I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd_max_length: Option<u64>,

    /// Maximum write I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr: Option<i64>,

    /// Maximum unthrottled write I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr_max: Option<i64>,

    /// Maximum length of write I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr_max_length: Option<u64>,

    /// Maximum r/w speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps: Option<f64>,

    /// Maximum unthrottled r/w pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_max: Option<f64>,

    /// Maximum read speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_rd: Option<f64>,

    /// Maximum unthrottled read pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_rd_max: Option<f64>,

    /// Maximum write speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_wr: Option<f64>,

    /// Maximum unthrottled write pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_wr_max: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<PveQmIdeMedia>,

    /// The drive's reported model name, url-encoded, up to 40 bytes long.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Whether the drive should considered for replication jobs.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicate: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerror: Option<PveQmIdeRerror>,

    /// The drive's reported serial number, url-encoded, up to 20 bytes long.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,

    /// Mark this locally-managed volume as available on all nodes
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared: Option<bool>,

    /// Disk size. This is purely informational and has no effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    /// Controls qemu's snapshot mode feature. If activated, changes made to the
    /// disk are temporary and will be discarded when the VM is shutdown.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<bool>,

    /// Whether to expose this drive as an SSD, rather than a rotational hard
    /// disk.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssd: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub werror: Option<PveQmIdeWerror>,

    /// The drive's worldwide name, encoded as 16 bytes hex string, prefixed by
    /// '0x'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wwn: Option<String>,
}

#[api]
/// AIO type to use.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum PveQmIdeAio {
    #[serde(rename = "native")]
    /// native.
    Native,
    #[serde(rename = "threads")]
    /// threads.
    Threads,
    #[serde(rename = "io_uring")]
    /// io_uring.
    IoUring,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(PveQmIdeAio);
serde_plain::derive_fromstr_from_deserialize!(PveQmIdeAio);

#[api]
/// The drive's cache mode
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum PveQmIdeCache {
    #[serde(rename = "none")]
    /// none.
    None,
    #[serde(rename = "writethrough")]
    /// writethrough.
    Writethrough,
    #[serde(rename = "writeback")]
    /// writeback.
    Writeback,
    #[serde(rename = "unsafe")]
    /// unsafe.
    Unsafe,
    #[serde(rename = "directsync")]
    /// directsync.
    Directsync,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(PveQmIdeCache);
serde_plain::derive_fromstr_from_deserialize!(PveQmIdeCache);

#[api]
/// Controls whether to pass discard/trim requests to the underlying storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum PveQmIdeDiscard {
    #[serde(rename = "ignore")]
    /// ignore.
    Ignore,
    #[serde(rename = "on")]
    /// on.
    On,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(PveQmIdeDiscard);
serde_plain::derive_fromstr_from_deserialize!(PveQmIdeDiscard);

#[api]
/// The drive's backing file's data format.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum PveQmIdeFormat {
    #[serde(rename = "raw")]
    /// raw.
    Raw,
    #[serde(rename = "qcow")]
    /// qcow.
    Qcow,
    #[serde(rename = "qed")]
    /// qed.
    Qed,
    #[serde(rename = "qcow2")]
    /// qcow2.
    Qcow2,
    #[serde(rename = "vmdk")]
    /// vmdk.
    Vmdk,
    #[serde(rename = "cloop")]
    /// cloop.
    Cloop,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(PveQmIdeFormat);
serde_plain::derive_fromstr_from_deserialize!(PveQmIdeFormat);

#[api]
/// The drive's media type.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum PveQmIdeMedia {
    #[serde(rename = "cdrom")]
    /// cdrom.
    Cdrom,
    #[serde(rename = "disk")]
    #[default]
    /// disk.
    Disk,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(PveQmIdeMedia);
serde_plain::derive_fromstr_from_deserialize!(PveQmIdeMedia);

#[api]
/// Read error action.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum PveQmIdeRerror {
    #[serde(rename = "ignore")]
    /// ignore.
    Ignore,
    #[serde(rename = "report")]
    /// report.
    Report,
    #[serde(rename = "stop")]
    /// stop.
    Stop,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(PveQmIdeRerror);
serde_plain::derive_fromstr_from_deserialize!(PveQmIdeRerror);

#[api]
/// Write error action.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum PveQmIdeWerror {
    #[serde(rename = "enospc")]
    /// enospc.
    Enospc,
    #[serde(rename = "ignore")]
    /// ignore.
    Ignore,
    #[serde(rename = "report")]
    /// report.
    Report,
    #[serde(rename = "stop")]
    /// stop.
    Stop,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(PveQmIdeWerror);
serde_plain::derive_fromstr_from_deserialize!(PveQmIdeWerror);

#[api(
    properties: {
        gw: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_ipv4),
            optional: true,
            type: String,
        },
        gw6: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_ipv6),
            optional: true,
            type: String,
        },
        ip: {
            default: "dhcp",
            format: &ApiStringFormat::VerifyFn(verifiers::verify_ipv4_config),
            optional: true,
            type: String,
        },
        ip6: {
            default: "dhcp",
            format: &ApiStringFormat::VerifyFn(verifiers::verify_ipv6_config),
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PveQmIpconfig {
    /// Default gateway for IPv4 traffic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gw: Option<String>,

    /// Default gateway for IPv6 traffic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gw6: Option<String>,

    /// IPv4 address in CIDR format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,

    /// IPv6 address in CIDR format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip6: Option<String>,
}

#[api(
    default_key: "source",
    properties: {
        max_bytes: {
            default: 1024,
            optional: true,
            type: Integer,
        },
        period: {
            default: 1000,
            optional: true,
            type: Integer,
        },
        source: {
            type: PveQmRngSource,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PveQmRng {
    /// Maximum bytes of entropy allowed to get injected into the guest every
    /// 'period' milliseconds. Use `0` to disable limiting (potentially
    /// dangerous!).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<i64>,

    /// Every 'period' milliseconds the entropy-injection quota is reset,
    /// allowing the guest to retrieve another 'max_bytes' of entropy.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<i64>,

    pub source: PveQmRngSource,
}

#[api]
/// The file on the host to gather entropy from. Using urandom does *not*
/// decrease security in any meaningful way, as it's still seeded from real
/// entropy, and the bytes provided will most likely be mixed with real entropy
/// on the guest as well. '/dev/hwrng' can be used to pass through a hardware
/// RNG from the host.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum PveQmRngSource {
    #[serde(rename = "/dev/urandom")]
    /// /dev/urandom.
    DevUrandom,
    #[serde(rename = "/dev/random")]
    /// /dev/random.
    DevRandom,
    #[serde(rename = "/dev/hwrng")]
    /// /dev/hwrng.
    DevHwrng,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(PveQmRngSource);
serde_plain::derive_fromstr_from_deserialize!(PveQmRngSource);

#[api(
    properties: {
        base64: {
            default: false,
            optional: true,
        },
        family: {
            optional: true,
            type: String,
        },
        manufacturer: {
            optional: true,
            type: String,
        },
        product: {
            optional: true,
            type: String,
        },
        serial: {
            optional: true,
            type: String,
        },
        sku: {
            optional: true,
            type: String,
        },
        uuid: {
            optional: true,
            type: String,
        },
        version: {
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PveQmSmbios1 {
    /// Flag to indicate that the SMBIOS values are base64 encoded
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base64: Option<bool>,

    /// Set SMBIOS1 family string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,

    /// Set SMBIOS1 manufacturer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,

    /// Set SMBIOS1 product ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,

    /// Set SMBIOS1 serial number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,

    /// Set SMBIOS1 SKU string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sku: Option<String>,

    /// Set SMBIOS1 UUID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,

    /// Set SMBIOS1 version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[api(
    default_key: "model",
    properties: {
        action: {
            optional: true,
            type: PveQmWatchdogAction,
        },
        model: {
            optional: true,
            type: PveQmWatchdogModel,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PveQmWatchdog {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<PveQmWatchdogAction>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<PveQmWatchdogModel>,
}

#[api]
/// The action to perform if after activation the guest fails to poll the
/// watchdog in time.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum PveQmWatchdogAction {
    #[serde(rename = "reset")]
    /// reset.
    Reset,
    #[serde(rename = "shutdown")]
    /// shutdown.
    Shutdown,
    #[serde(rename = "poweroff")]
    /// poweroff.
    Poweroff,
    #[serde(rename = "pause")]
    /// pause.
    Pause,
    #[serde(rename = "debug")]
    /// debug.
    Debug,
    #[serde(rename = "none")]
    /// none.
    None,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(PveQmWatchdogAction);
serde_plain::derive_fromstr_from_deserialize!(PveQmWatchdogAction);

#[api]
/// Watchdog type to emulate.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum PveQmWatchdogModel {
    #[serde(rename = "i6300esb")]
    #[default]
    /// i6300esb.
    I6300esb,
    #[serde(rename = "ib700")]
    /// ib700.
    Ib700,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(PveQmWatchdogModel);
serde_plain::derive_fromstr_from_deserialize!(PveQmWatchdogModel);

#[api(
    default_key: "cputype",
    properties: {
        cputype: {
            default: "kvm64",
            optional: true,
            type: String,
        },
        flags: {
            optional: true,
            type: String,
        },
        "guest-phys-bits": {
            maximum: 64,
            minimum: 32,
            optional: true,
            type: Integer,
        },
        hidden: {
            default: false,
            optional: true,
        },
        "hv-vendor-id": {
            optional: true,
            type: String,
        },
        "phys-bits": {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_phys_bits),
            optional: true,
            type: String,
        },
        "reported-model": {
            optional: true,
            type: PveVmCpuConfReportedModel,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PveVmCpuConf {
    /// Emulated CPU type. Can be default or custom name (custom model names
    /// must be prefixed with 'custom-').
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cputype: Option<String>,

    /// List of additional CPU flags separated by ';'. Use '+FLAG' to enable,
    /// '-FLAG' to disable a flag. Custom CPU models can specify any flag
    /// supported by QEMU/KVM, VM-specific flags must be from the following set
    /// for security reasons: pcid, spec-ctrl, ibpb, ssbd, virt-ssbd, amd-ssbd,
    /// amd-no-ssb, pdpe1gb, md-clear, hv-tlbflush, hv-evmcs, aes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<String>,

    /// Number of physical address bits available to the guest.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u8")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "guest-phys-bits")]
    pub guest_phys_bits: Option<u8>,

    /// Do not identify as a KVM virtual machine.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,

    /// The Hyper-V vendor ID. Some drivers or programs inside Windows guests
    /// need a specific ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "hv-vendor-id")]
    pub hv_vendor_id: Option<String>,

    /// The physical memory address bits that are reported to the guest OS.
    /// Should be smaller or equal to the host's. Set to 'host' to use value
    /// from host CPU, but note that doing so will break live migration to CPUs
    /// with other values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "phys-bits")]
    pub phys_bits: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "reported-model")]
    pub reported_model: Option<PveVmCpuConfReportedModel>,
}

#[api]
/// CPU model and vendor to report to the guest. Must be a QEMU/KVM supported
/// model. Only valid for custom CPU model definitions, default models will
/// always report themselves to the guest OS.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum PveVmCpuConfReportedModel {
    #[serde(rename = "486")]
    /// 486.
    I486,
    #[serde(rename = "athlon")]
    /// athlon.
    Athlon,
    /// Broadwell.
    Broadwell,
    #[serde(rename = "Broadwell-IBRS")]
    /// Broadwell-IBRS.
    BroadwellIbrs,
    #[serde(rename = "Broadwell-noTSX")]
    /// Broadwell-noTSX.
    BroadwellNoTsx,
    #[serde(rename = "Broadwell-noTSX-IBRS")]
    /// Broadwell-noTSX-IBRS.
    BroadwellNoTsxIbrs,
    #[serde(rename = "Cascadelake-Server")]
    /// Cascadelake-Server.
    CascadelakeServer,
    #[serde(rename = "Cascadelake-Server-noTSX")]
    /// Cascadelake-Server-noTSX.
    CascadelakeServerNoTsx,
    #[serde(rename = "Cascadelake-Server-v2")]
    /// Cascadelake-Server-v2.
    CascadelakeServerV2,
    #[serde(rename = "Cascadelake-Server-v4")]
    /// Cascadelake-Server-v4.
    CascadelakeServerV4,
    #[serde(rename = "Cascadelake-Server-v5")]
    /// Cascadelake-Server-v5.
    CascadelakeServerV5,
    /// Conroe.
    Conroe,
    /// Cooperlake.
    Cooperlake,
    #[serde(rename = "Cooperlake-v2")]
    /// Cooperlake-v2.
    CooperlakeV2,
    #[serde(rename = "core2duo")]
    /// core2duo.
    Core2duo,
    #[serde(rename = "coreduo")]
    /// coreduo.
    Coreduo,
    #[serde(rename = "EPYC")]
    /// EPYC.
    Epyc,
    #[serde(rename = "EPYC-Genoa")]
    /// EPYC-Genoa.
    EpycGenoa,
    #[serde(rename = "EPYC-IBPB")]
    /// EPYC-IBPB.
    EpycIbpb,
    #[serde(rename = "EPYC-Milan")]
    /// EPYC-Milan.
    EpycMilan,
    #[serde(rename = "EPYC-Milan-v2")]
    /// EPYC-Milan-v2.
    EpycMilanV2,
    #[serde(rename = "EPYC-Rome")]
    /// EPYC-Rome.
    EpycRome,
    #[serde(rename = "EPYC-Rome-v2")]
    /// EPYC-Rome-v2.
    EpycRomeV2,
    #[serde(rename = "EPYC-Rome-v3")]
    /// EPYC-Rome-v3.
    EpycRomeV3,
    #[serde(rename = "EPYC-Rome-v4")]
    /// EPYC-Rome-v4.
    EpycRomeV4,
    #[serde(rename = "EPYC-v3")]
    /// EPYC-v3.
    EpycV3,
    #[serde(rename = "EPYC-v4")]
    /// EPYC-v4.
    EpycV4,
    /// GraniteRapids.
    GraniteRapids,
    /// Haswell.
    Haswell,
    #[serde(rename = "Haswell-IBRS")]
    /// Haswell-IBRS.
    HaswellIbrs,
    #[serde(rename = "Haswell-noTSX")]
    /// Haswell-noTSX.
    HaswellNoTsx,
    #[serde(rename = "Haswell-noTSX-IBRS")]
    /// Haswell-noTSX-IBRS.
    HaswellNoTsxIbrs,
    #[serde(rename = "host")]
    /// host.
    Host,
    #[serde(rename = "Icelake-Client")]
    /// Icelake-Client.
    IcelakeClient,
    #[serde(rename = "Icelake-Client-noTSX")]
    /// Icelake-Client-noTSX.
    IcelakeClientNoTsx,
    #[serde(rename = "Icelake-Server")]
    /// Icelake-Server.
    IcelakeServer,
    #[serde(rename = "Icelake-Server-noTSX")]
    /// Icelake-Server-noTSX.
    IcelakeServerNoTsx,
    #[serde(rename = "Icelake-Server-v3")]
    /// Icelake-Server-v3.
    IcelakeServerV3,
    #[serde(rename = "Icelake-Server-v4")]
    /// Icelake-Server-v4.
    IcelakeServerV4,
    #[serde(rename = "Icelake-Server-v5")]
    /// Icelake-Server-v5.
    IcelakeServerV5,
    #[serde(rename = "Icelake-Server-v6")]
    /// Icelake-Server-v6.
    IcelakeServerV6,
    /// IvyBridge.
    IvyBridge,
    #[serde(rename = "IvyBridge-IBRS")]
    /// IvyBridge-IBRS.
    IvyBridgeIbrs,
    /// KnightsMill.
    KnightsMill,
    #[serde(rename = "kvm32")]
    /// kvm32.
    Kvm32,
    #[serde(rename = "kvm64")]
    #[default]
    /// kvm64.
    Kvm64,
    #[serde(rename = "max")]
    /// max.
    Max,
    /// Nehalem.
    Nehalem,
    #[serde(rename = "Nehalem-IBRS")]
    /// Nehalem-IBRS.
    NehalemIbrs,
    #[serde(rename = "Opteron_G1")]
    /// Opteron_G1.
    OpteronG1,
    #[serde(rename = "Opteron_G2")]
    /// Opteron_G2.
    OpteronG2,
    #[serde(rename = "Opteron_G3")]
    /// Opteron_G3.
    OpteronG3,
    #[serde(rename = "Opteron_G4")]
    /// Opteron_G4.
    OpteronG4,
    #[serde(rename = "Opteron_G5")]
    /// Opteron_G5.
    OpteronG5,
    /// Penryn.
    Penryn,
    #[serde(rename = "pentium")]
    /// pentium.
    Pentium,
    #[serde(rename = "pentium2")]
    /// pentium2.
    Pentium2,
    #[serde(rename = "pentium3")]
    /// pentium3.
    Pentium3,
    #[serde(rename = "phenom")]
    /// phenom.
    Phenom,
    #[serde(rename = "qemu32")]
    /// qemu32.
    Qemu32,
    #[serde(rename = "qemu64")]
    /// qemu64.
    Qemu64,
    /// SandyBridge.
    SandyBridge,
    #[serde(rename = "SandyBridge-IBRS")]
    /// SandyBridge-IBRS.
    SandyBridgeIbrs,
    /// SapphireRapids.
    SapphireRapids,
    #[serde(rename = "SapphireRapids-v2")]
    /// SapphireRapids-v2.
    SapphireRapidsV2,
    #[serde(rename = "Skylake-Client")]
    /// Skylake-Client.
    SkylakeClient,
    #[serde(rename = "Skylake-Client-IBRS")]
    /// Skylake-Client-IBRS.
    SkylakeClientIbrs,
    #[serde(rename = "Skylake-Client-noTSX-IBRS")]
    /// Skylake-Client-noTSX-IBRS.
    SkylakeClientNoTsxIbrs,
    #[serde(rename = "Skylake-Client-v4")]
    /// Skylake-Client-v4.
    SkylakeClientV4,
    #[serde(rename = "Skylake-Server")]
    /// Skylake-Server.
    SkylakeServer,
    #[serde(rename = "Skylake-Server-IBRS")]
    /// Skylake-Server-IBRS.
    SkylakeServerIbrs,
    #[serde(rename = "Skylake-Server-noTSX-IBRS")]
    /// Skylake-Server-noTSX-IBRS.
    SkylakeServerNoTsxIbrs,
    #[serde(rename = "Skylake-Server-v4")]
    /// Skylake-Server-v4.
    SkylakeServerV4,
    #[serde(rename = "Skylake-Server-v5")]
    /// Skylake-Server-v5.
    SkylakeServerV5,
    /// Westmere.
    Westmere,
    #[serde(rename = "Westmere-IBRS")]
    /// Westmere-IBRS.
    WestmereIbrs,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(PveVmCpuConfReportedModel);
serde_plain::derive_fromstr_from_deserialize!(PveVmCpuConfReportedModel);

const_regex! {

QEMU_CONFIG_AFFINITY_RE = r##"^(\s*\d+(-\d+)?\s*)(,\s*\d+(-\d+)?\s*)?$"##;
QEMU_CONFIG_BOOTDISK_RE = r##"^(ide|sata|scsi|virtio|efidisk|tpmstate)\d+$"##;
QEMU_CONFIG_PARENT_RE = r##"^(?i:[a-z][a-z0-9_-]+)$"##;
QEMU_CONFIG_SSHKEYS_RE = r##"^[-%a-zA-Z0-9_.!~*'()]*$"##;
QEMU_CONFIG_TAGS_RE = r##"^(?i)[a-z0-9_][a-z0-9_\-+.]*$"##;
QEMU_CONFIG_VMSTATESTORAGE_RE = r##"^(?i:[a-z][a-z0-9\-_.]*[a-z0-9])$"##;

}

#[test]
fn test_regex_compilation_17() {
    use regex::Regex;
    let _: &Regex = &QEMU_CONFIG_AFFINITY_RE;
    let _: &Regex = &QEMU_CONFIG_BOOTDISK_RE;
    let _: &Regex = &QEMU_CONFIG_PARENT_RE;
    let _: &Regex = &QEMU_CONFIG_SSHKEYS_RE;
    let _: &Regex = &QEMU_CONFIG_TAGS_RE;
    let _: &Regex = &QEMU_CONFIG_VMSTATESTORAGE_RE;
}
#[api(
    properties: {
        acpi: {
            default: true,
            optional: true,
        },
        affinity: {
            format: &ApiStringFormat::Pattern(&QEMU_CONFIG_AFFINITY_RE),
            optional: true,
            type: String,
        },
        agent: {
            format: &ApiStringFormat::PropertyString(&QemuConfigAgent::API_SCHEMA),
            optional: true,
            type: String,
        },
        "amd-sev": {
            format: &ApiStringFormat::PropertyString(&PveQemuSevFmt::API_SCHEMA),
            optional: true,
            type: String,
        },
        arch: {
            optional: true,
            type: QemuConfigArch,
        },
        args: {
            optional: true,
            type: String,
        },
        audio0: {
            format: &ApiStringFormat::PropertyString(&QemuConfigAudio0::API_SCHEMA),
            optional: true,
            type: String,
        },
        autostart: {
            default: false,
            optional: true,
        },
        balloon: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        bios: {
            optional: true,
            type: QemuConfigBios,
        },
        boot: {
            format: &ApiStringFormat::PropertyString(&PveQmBoot::API_SCHEMA),
            optional: true,
            type: String,
        },
        bootdisk: {
            format: &ApiStringFormat::Pattern(&QEMU_CONFIG_BOOTDISK_RE),
            optional: true,
            type: String,
        },
        cdrom: {
            format: &ApiStringFormat::PropertyString(&PveQmIde::API_SCHEMA),
            optional: true,
            type: String,
            type_text: "<volume>",
        },
        cicustom: {
            format: &ApiStringFormat::PropertyString(&PveQmCicustom::API_SCHEMA),
            optional: true,
            type: String,
        },
        cipassword: {
            optional: true,
            type: String,
        },
        citype: {
            optional: true,
            type: QemuConfigCitype,
        },
        ciupgrade: {
            default: true,
            optional: true,
        },
        ciuser: {
            optional: true,
            type: String,
        },
        cores: {
            default: 1,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        cpu: {
            format: &ApiStringFormat::PropertyString(&PveVmCpuConf::API_SCHEMA),
            optional: true,
            type: String,
        },
        cpulimit: {
            default: 0.0,
            maximum: 128.0,
            minimum: 0.0,
            optional: true,
        },
        cpuunits: {
            default: 1024,
            maximum: 262144,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        description: {
            max_length: 8192,
            optional: true,
            type: String,
        },
        digest: {
            type: String,
        },
        efidisk0: {
            format: &ApiStringFormat::PropertyString(&QemuConfigEfidisk0::API_SCHEMA),
            optional: true,
            type: String,
        },
        freeze: {
            default: false,
            optional: true,
        },
        hookscript: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_volume_id),
            optional: true,
            type: String,
        },
        hostpci: {
            type: QemuConfigHostpciArray,
        },
        hotplug: {
            default: "network,disk,usb",
            optional: true,
            type: String,
        },
        hugepages: {
            optional: true,
            type: QemuConfigHugepages,
        },
        ide: {
            type: QemuConfigIdeArray,
        },
        ipconfig: {
            type: QemuConfigIpconfigArray,
        },
        ivshmem: {
            format: &ApiStringFormat::PropertyString(&QemuConfigIvshmem::API_SCHEMA),
            optional: true,
            type: String,
        },
        keephugepages: {
            default: false,
            optional: true,
        },
        keyboard: {
            optional: true,
            type: QemuConfigKeyboard,
        },
        kvm: {
            default: true,
            optional: true,
        },
        localtime: {
            default: false,
            optional: true,
        },
        lock: {
            optional: true,
            type: QemuConfigLock,
        },
        machine: {
            format: &ApiStringFormat::PropertyString(&QemuConfigMachine::API_SCHEMA),
            optional: true,
            type: String,
        },
        memory: {
            format: &ApiStringFormat::PropertyString(&QemuConfigMemory::API_SCHEMA),
            optional: true,
            type: String,
        },
        meta: {
            format: &ApiStringFormat::PropertyString(&QemuConfigMeta::API_SCHEMA),
            optional: true,
            type: String,
        },
        migrate_downtime: {
            default: 0.1,
            minimum: 0.0,
            optional: true,
        },
        migrate_speed: {
            default: 0,
            minimum: 0,
            optional: true,
            type: Integer,
        },
        name: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_dns_name),
            optional: true,
            type: String,
        },
        nameserver: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_address),
            optional: true,
            type: String,
        },
        net: {
            type: QemuConfigNetArray,
        },
        numa: {
            default: false,
            optional: true,
        },
        numa_array: {
            type: QemuConfigNumaArray,
        },
        onboot: {
            default: false,
            optional: true,
        },
        ostype: {
            optional: true,
            type: QemuConfigOstype,
        },
        parallel: {
            type: QemuConfigParallelArray,
        },
        parent: {
            format: &ApiStringFormat::Pattern(&QEMU_CONFIG_PARENT_RE),
            max_length: 40,
            optional: true,
            type: String,
        },
        protection: {
            default: false,
            optional: true,
        },
        reboot: {
            default: true,
            optional: true,
        },
        rng0: {
            format: &ApiStringFormat::PropertyString(&PveQmRng::API_SCHEMA),
            optional: true,
            type: String,
        },
        "running-nets-host-mtu": {
            optional: true,
            type: String,
        },
        runningcpu: {
            optional: true,
            type: String,
        },
        runningmachine: {
            format: &ApiStringFormat::PropertyString(&QemuConfigMachine::API_SCHEMA),
            optional: true,
            type: String,
        },
        sata: {
            type: QemuConfigSataArray,
        },
        scsi: {
            type: QemuConfigScsiArray,
        },
        scsihw: {
            optional: true,
            type: QemuConfigScsihw,
        },
        searchdomain: {
            optional: true,
            type: String,
        },
        serial: {
            type: QemuConfigSerialArray,
        },
        shares: {
            default: 1000,
            maximum: 50000,
            minimum: 0,
            optional: true,
            type: Integer,
        },
        smbios1: {
            format: &ApiStringFormat::PropertyString(&PveQmSmbios1::API_SCHEMA),
            max_length: 512,
            optional: true,
            type: String,
        },
        smp: {
            default: 1,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        snaptime: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        sockets: {
            default: 1,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        spice_enhancements: {
            format: &ApiStringFormat::PropertyString(&QemuConfigSpiceEnhancements::API_SCHEMA),
            optional: true,
            type: String,
        },
        sshkeys: {
            format: &ApiStringFormat::Pattern(&QEMU_CONFIG_SSHKEYS_RE),
            optional: true,
            type: String,
        },
        startdate: {
            default: "now",
            optional: true,
            type: String,
            type_text: "(now | YYYY-MM-DD | YYYY-MM-DDTHH:MM:SS)",
        },
        startup: {
            optional: true,
            type: String,
            type_text: "[[order=]\\d+] [,up=\\d+] [,down=\\d+] ",
        },
        tablet: {
            default: true,
            optional: true,
        },
        tags: {
            format: &ApiStringFormat::Pattern(&QEMU_CONFIG_TAGS_RE),
            optional: true,
            type: String,
        },
        tdf: {
            default: false,
            optional: true,
        },
        template: {
            default: false,
            optional: true,
        },
        tpmstate0: {
            format: &ApiStringFormat::PropertyString(&QemuConfigTpmstate0::API_SCHEMA),
            optional: true,
            type: String,
        },
        unused: {
            type: QemuConfigUnusedArray,
        },
        usb: {
            type: QemuConfigUsbArray,
        },
        vcpus: {
            default: 0,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        vga: {
            format: &ApiStringFormat::PropertyString(&QemuConfigVga::API_SCHEMA),
            optional: true,
            type: String,
        },
        virtio: {
            type: QemuConfigVirtioArray,
        },
        virtiofs: {
            type: QemuConfigVirtiofsArray,
        },
        vmgenid: {
            default: "1 (autogenerated)",
            optional: true,
            type: String,
        },
        vmstate: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_volume_id),
            optional: true,
            type: String,
        },
        vmstatestorage: {
            format: &ApiStringFormat::Pattern(&QEMU_CONFIG_VMSTATESTORAGE_RE),
            optional: true,
            type: String,
        },
        watchdog: {
            format: &ApiStringFormat::PropertyString(&PveQmWatchdog::API_SCHEMA),
            optional: true,
            type: String,
        },
    },
)]
/// The VM configuration.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfig {
    /// Enable/disable ACPI.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acpi: Option<bool>,

    /// List of host cores used to execute guest processes, for example:
    /// 0,5,8-11
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affinity: Option<String>,

    /// Enable/disable communication with the QEMU Guest Agent and its
    /// properties.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,

    /// Secure Encrypted Virtualization (SEV) features by AMD CPUs
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "amd-sev")]
    pub amd_sev: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<QemuConfigArch>,

    /// Arbitrary arguments passed to kvm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,

    /// Configure a audio device, useful in combination with QXL/Spice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio0: Option<String>,

    /// Automatic restart after crash (currently ignored).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autostart: Option<bool>,

    /// Amount of target RAM for the VM in MiB. Using zero disables the ballon
    /// driver.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balloon: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bios: Option<QemuConfigBios>,

    /// Specify guest boot order. Use the 'order=' sub-property as usage with no
    /// key or 'legacy=' is deprecated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boot: Option<String>,

    /// Enable booting from specified disk. Deprecated: Use 'boot:
    /// order=foo;bar' instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootdisk: Option<String>,

    /// This is an alias for option -ide2
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cdrom: Option<String>,

    /// cloud-init: Specify custom files to replace the automatically generated
    /// ones at start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cicustom: Option<String>,

    /// cloud-init: Password to assign the user. Using this is generally not
    /// recommended. Use ssh keys instead. Also note that older cloud-init
    /// versions do not support hashed passwords.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cipassword: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citype: Option<QemuConfigCitype>,

    /// cloud-init: do an automatic package upgrade after the first boot.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ciupgrade: Option<bool>,

    /// cloud-init: User name to change ssh keys and password for instead of the
    /// image's configured default user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ciuser: Option<String>,

    /// The number of cores per socket.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cores: Option<u64>,

    /// Emulated CPU type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<String>,

    /// Limit of CPU usage.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpulimit: Option<f64>,

    /// CPU weight for a VM, will be clamped to [1, 10000] in cgroup v2.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpuunits: Option<u32>,

    /// Description for the VM. Shown in the web-interface VM's summary. This is
    /// saved as comment inside the configuration file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// SHA1 digest of configuration file. This can be used to prevent
    /// concurrent modifications.
    pub digest: String,

    /// Configure a disk for storing EFI vars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub efidisk0: Option<String>,

    /// Freeze CPU at startup (use 'c' monitor command to start execution).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze: Option<bool>,

    /// Script that will be executed during various steps in the vms lifetime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hookscript: Option<String>,

    /// Map host PCI devices into guest.
    #[serde(flatten)]
    pub hostpci: QemuConfigHostpciArray,

    /// Selectively enable hotplug features. This is a comma separated list of
    /// hotplug features: 'network', 'disk', 'cpu', 'memory', 'usb' and
    /// 'cloudinit'. Use '0' to disable hotplug completely. Using '1' as value
    /// is an alias for the default `network,disk,usb`. USB hotplugging is
    /// possible for guests with machine version >= 7.1 and ostype l26 or
    /// windows > 7.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotplug: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hugepages: Option<QemuConfigHugepages>,

    /// Use volume as IDE hard disk or CD-ROM (n is 0 to 3).
    #[serde(flatten)]
    pub ide: QemuConfigIdeArray,

    /// cloud-init: Specify IP addresses and gateways for the corresponding
    /// interface.
    ///
    /// IP addresses use CIDR notation, gateways are optional but need an IP of
    /// the same type specified.
    ///
    /// The special string 'dhcp' can be used for IP addresses to use DHCP, in
    /// which case no explicit gateway should be provided.
    /// For IPv6 the special string 'auto' can be used to use stateless
    /// autoconfiguration. This requires cloud-init 19.4 or newer.
    ///
    /// If cloud-init is enabled and neither an IPv4 nor an IPv6 address is
    /// specified, it defaults to using dhcp on IPv4.
    #[serde(flatten)]
    pub ipconfig: QemuConfigIpconfigArray,

    /// Inter-VM shared memory. Useful for direct communication between VMs, or
    /// to the host.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ivshmem: Option<String>,

    /// Use together with hugepages. If enabled, hugepages will not not be
    /// deleted after VM shutdown and can be used for subsequent starts.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keephugepages: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyboard: Option<QemuConfigKeyboard>,

    /// Enable/disable KVM hardware virtualization.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kvm: Option<bool>,

    /// Set the real time clock (RTC) to local time. This is enabled by default
    /// if the `ostype` indicates a Microsoft Windows OS.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub localtime: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<QemuConfigLock>,

    /// Specify the QEMU machine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,

    /// Memory properties.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,

    /// Some (read-only) meta-information about this guest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<String>,

    /// Set maximum tolerated downtime (in seconds) for migrations. Should the
    /// migration not be able to converge in the very end, because too much
    /// newly dirtied RAM needs to be transferred, the limit will be increased
    /// automatically step-by-step until migration can converge.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migrate_downtime: Option<f64>,

    /// Set maximum speed (in MB/s) for migrations. Value 0 is no limit.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migrate_speed: Option<u64>,

    /// Set a name for the VM. Only used on the configuration web interface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// cloud-init: Sets DNS server IP address for a container. Create will
    /// automatically use the setting from the host if neither searchdomain nor
    /// nameserver are set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nameserver: Option<String>,

    /// Specify network devices.
    #[serde(flatten)]
    pub net: QemuConfigNetArray,

    /// Enable/disable NUMA.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numa: Option<bool>,

    /// NUMA topology.
    #[serde(flatten)]
    pub numa_array: QemuConfigNumaArray,

    /// Specifies whether a VM will be started during system bootup.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onboot: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ostype: Option<QemuConfigOstype>,

    /// Map host parallel devices (n is 0 to 2).
    #[serde(flatten)]
    pub parallel: QemuConfigParallelArray,

    /// Parent snapshot name. This is used internally, and should not be
    /// modified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,

    /// Sets the protection flag of the VM. This will disable the remove VM and
    /// remove disk operations.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protection: Option<bool>,

    /// Allow reboot. If set to '0' the VM exit on reboot.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reboot: Option<bool>,

    /// Configure a VirtIO-based Random Number Generator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rng0: Option<String>,

    /// List of VirtIO network devices and their effective host_mtu setting. A
    /// value of 0 means that the host_mtu parameter is to be avoided for the
    /// corresponding device. This is used internally for snapshots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "running-nets-host-mtu")]
    pub running_nets_host_mtu: Option<String>,

    /// Specifies the QEMU '-cpu' parameter of the running vm. This is used
    /// internally for snapshots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runningcpu: Option<String>,

    /// Specifies the QEMU machine type of the running vm. This is used
    /// internally for snapshots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runningmachine: Option<String>,

    /// Use volume as SATA hard disk or CD-ROM (n is 0 to 5).
    #[serde(flatten)]
    pub sata: QemuConfigSataArray,

    /// Use volume as SCSI hard disk or CD-ROM (n is 0 to 30).
    #[serde(flatten)]
    pub scsi: QemuConfigScsiArray,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scsihw: Option<QemuConfigScsihw>,

    /// cloud-init: Sets DNS search domains for a container. Create will
    /// automatically use the setting from the host if neither searchdomain nor
    /// nameserver are set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub searchdomain: Option<String>,

    /// Create a serial device inside the VM (n is 0 to 3)
    #[serde(flatten)]
    pub serial: QemuConfigSerialArray,

    /// Amount of memory shares for auto-ballooning. The larger the number is,
    /// the more memory this VM gets. Number is relative to weights of all other
    /// running VMs. Using zero disables auto-ballooning. Auto-ballooning is
    /// done by pvestatd.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u16")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shares: Option<u16>,

    /// Specify SMBIOS type 1 fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smbios1: Option<String>,

    /// The number of CPUs. Please use option -sockets instead.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smp: Option<u64>,

    /// Timestamp for snapshots.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snaptime: Option<u64>,

    /// The number of CPU sockets.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sockets: Option<u64>,

    /// Configure additional enhancements for SPICE.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spice_enhancements: Option<String>,

    /// cloud-init: Setup public SSH keys (one key per line, OpenSSH format).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sshkeys: Option<String>,

    /// Set the initial date of the real time clock. Valid format for date
    /// are:'now' or '2006-06-17T16:01:21' or '2006-06-17'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startdate: Option<String>,

    /// Startup and shutdown behavior. Order is a non-negative number defining
    /// the general startup order. Shutdown in done with reverse ordering.
    /// Additionally you can set the 'up' or 'down' delay in seconds, which
    /// specifies a delay to wait before the next VM is started or stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startup: Option<String>,

    /// Enable/disable the USB tablet device.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tablet: Option<bool>,

    /// Tags of the VM. This is only meta information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,

    /// Enable/disable time drift fix.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tdf: Option<bool>,

    /// Enable/disable Template.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<bool>,

    /// Configure a Disk for storing TPM state. The format is fixed to 'raw'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tpmstate0: Option<String>,

    /// Reference to unused volumes. This is used internally, and should not be
    /// modified manually.
    #[serde(flatten)]
    pub unused: QemuConfigUnusedArray,

    /// Configure an USB device (n is 0 to 4, for machine version >= 7.1 and
    /// ostype l26 or windows > 7, n can be up to 14).
    #[serde(flatten)]
    pub usb: QemuConfigUsbArray,

    /// Number of hotplugged vcpus.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcpus: Option<u64>,

    /// Configure the VGA hardware.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vga: Option<String>,

    /// Use volume as VIRTIO hard disk (n is 0 to 15).
    #[serde(flatten)]
    pub virtio: QemuConfigVirtioArray,

    /// Configuration for sharing a directory between host and guest using
    /// Virtio-fs.
    #[serde(flatten)]
    pub virtiofs: QemuConfigVirtiofsArray,

    /// Set VM Generation ID. Use '1' to autogenerate on create or update, pass
    /// '0' to disable explicitly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vmgenid: Option<String>,

    /// Reference to a volume which stores the VM state. This is used internally
    /// for snapshots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vmstate: Option<String>,

    /// Default storage for VM state volumes/files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vmstatestorage: Option<String>,

    /// Create a virtual hardware watchdog device.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watchdog: Option<String>,
}
generate_array_field! {
    QemuConfigHostpciArray [ 16 ] :
    r#"Map host PCI devices into guest."#,
    String => {
        description: "Map host PCI devices into guest.",
        format: &ApiStringFormat::PropertyString(&PveQmHostpci::API_SCHEMA),
        type: String,
    }
    hostpci
}
generate_array_field! {
    QemuConfigIdeArray [ 4 ] :
    r#"Use volume as IDE hard disk or CD-ROM (n is 0 to 3)."#,
    String => {
        description: "Use volume as IDE hard disk or CD-ROM (n is 0 to 3).",
        format: &ApiStringFormat::PropertyString(&PveQmIde::API_SCHEMA),
        type: String,
    }
    ide
}
generate_array_field! {
    QemuConfigIpconfigArray [ 32 ] :
    r#"cloud-init: Specify IP addresses and gateways for the corresponding interface.

IP addresses use CIDR notation, gateways are optional but need an IP of the same type specified.

The special string 'dhcp' can be used for IP addresses to use DHCP, in which case no explicit
gateway should be provided.
For IPv6 the special string 'auto' can be used to use stateless autoconfiguration. This requires
cloud-init 19.4 or newer.

If cloud-init is enabled and neither an IPv4 nor an IPv6 address is specified, it defaults to using
dhcp on IPv4."#,
    String => {
        description: "cloud-init: Specify IP addresses and gateways for the corresponding interface.

IP addresses use CIDR notation, gateways are optional but need an IP of the same type specified.

The special string 'dhcp' can be used for IP addresses to use DHCP, in which case no explicit
gateway should be provided.
For IPv6 the special string 'auto' can be used to use stateless autoconfiguration. This requires
cloud-init 19.4 or newer.

If cloud-init is enabled and neither an IPv4 nor an IPv6 address is specified, it defaults to using
dhcp on IPv4.
",
        format: &ApiStringFormat::PropertyString(&PveQmIpconfig::API_SCHEMA),
        type: String,
    }
    ipconfig
}
generate_array_field! {
    QemuConfigNetArray [ 32 ] :
    r#"Specify network devices."#,
    String => {
        description: "Specify network devices.",
        format: &ApiStringFormat::PropertyString(&QemuConfigNet::API_SCHEMA),
        type: String,
    }
    net
}
generate_array_field! {
    QemuConfigNumaArray [ 8 ] :
    r#"NUMA topology."#,
    String => {
        description: "NUMA topology.",
        format: &ApiStringFormat::PropertyString(&QemuConfigNuma::API_SCHEMA),
        type: String,
    }
    numa_array
}
generate_array_field! {
    QemuConfigParallelArray [ 3 ] :
    r#"Map host parallel devices (n is 0 to 2)."#,
    String => {
        description: "Map host parallel devices (n is 0 to 2).",
        type: String,
    }
    parallel
}
generate_array_field! {
    QemuConfigSataArray [ 6 ] :
    r#"Use volume as SATA hard disk or CD-ROM (n is 0 to 5)."#,
    String => {
        description: "Use volume as SATA hard disk or CD-ROM (n is 0 to 5).",
        format: &ApiStringFormat::PropertyString(&QemuConfigSata::API_SCHEMA),
        type: String,
    }
    sata
}
generate_array_field! {
    QemuConfigScsiArray [ 31 ] :
    r#"Use volume as SCSI hard disk or CD-ROM (n is 0 to 30)."#,
    String => {
        description: "Use volume as SCSI hard disk or CD-ROM (n is 0 to 30).",
        format: &ApiStringFormat::PropertyString(&QemuConfigScsi::API_SCHEMA),
        type: String,
    }
    scsi
}
generate_array_field! {
    QemuConfigSerialArray [ 4 ] :
    r#"Create a serial device inside the VM (n is 0 to 3)"#,
    String => {
        description: "Create a serial device inside the VM (n is 0 to 3)",
        type: String,
    }
    serial
}
generate_array_field! {
    QemuConfigUnusedArray [ 256 ] :
    r#"Reference to unused volumes. This is used internally, and should not be modified manually."#,
    String => {
        description: "Reference to unused volumes. This is used internally, and should not be modified manually.",
        format: &ApiStringFormat::PropertyString(&QemuConfigUnused::API_SCHEMA),
        type: String,
    }
    unused
}
generate_array_field! {
    QemuConfigUsbArray [ 14 ] :
    r#"Configure an USB device (n is 0 to 4, for machine version >= 7.1 and ostype l26 or windows > 7, n can be up to 14)."#,
    String => {
        description: "Configure an USB device (n is 0 to 4, for machine version >= 7.1 and ostype l26 or windows > 7, n can be up to 14).",
        format: &ApiStringFormat::PropertyString(&QemuConfigUsb::API_SCHEMA),
        type: String,
    }
    usb
}
generate_array_field! {
    QemuConfigVirtioArray [ 16 ] :
    r#"Use volume as VIRTIO hard disk (n is 0 to 15)."#,
    String => {
        description: "Use volume as VIRTIO hard disk (n is 0 to 15).",
        format: &ApiStringFormat::PropertyString(&QemuConfigVirtio::API_SCHEMA),
        type: String,
    }
    virtio
}
generate_array_field! {
    QemuConfigVirtiofsArray [ 10 ] :
    r#"Configuration for sharing a directory between host and guest using Virtio-fs."#,
    String => {
        description: "Configuration for sharing a directory between host and guest using Virtio-fs.",
        format: &ApiStringFormat::PropertyString(&QemuConfigVirtiofs::API_SCHEMA),
        type: String,
    }
    virtiofs
}

#[api(
    default_key: "enabled",
    properties: {
        enabled: {
            default: false,
        },
        "freeze-fs-on-backup": {
            default: true,
            optional: true,
        },
        fstrim_cloned_disks: {
            default: false,
            optional: true,
        },
        type: {
            optional: true,
            type: QemuConfigAgentType,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfigAgent {
    /// Enable/disable communication with a QEMU Guest Agent (QGA) running in
    /// the VM.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    pub enabled: bool,

    /// Freeze/thaw guest filesystems on backup for consistency.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "freeze-fs-on-backup")]
    pub freeze_fs_on_backup: Option<bool>,

    /// Run fstrim after moving a disk or migrating the VM.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fstrim_cloned_disks: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub ty: Option<QemuConfigAgentType>,
}

#[api]
/// Select the agent type
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigAgentType {
    #[serde(rename = "virtio")]
    #[default]
    /// virtio.
    Virtio,
    #[serde(rename = "isa")]
    /// isa.
    Isa,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigAgentType);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigAgentType);

#[api]
/// Virtual processor architecture. Defaults to the host.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigArch {
    #[serde(rename = "x86_64")]
    /// x86_64.
    X8664,
    #[serde(rename = "aarch64")]
    /// aarch64.
    Aarch64,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigArch);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigArch);

#[api(
    properties: {
        device: {
            type: QemuConfigAudio0Device,
        },
        driver: {
            optional: true,
            type: QemuConfigAudio0Driver,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfigAudio0 {
    pub device: QemuConfigAudio0Device,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver: Option<QemuConfigAudio0Driver>,
}

#[api]
/// Configure an audio device.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigAudio0Device {
    #[serde(rename = "ich9-intel-hda")]
    /// ich9-intel-hda.
    Ich9IntelHda,
    #[serde(rename = "intel-hda")]
    /// intel-hda.
    IntelHda,
    #[serde(rename = "AC97")]
    /// AC97.
    Ac97,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigAudio0Device);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigAudio0Device);

#[api]
/// Driver backend for the audio device.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigAudio0Driver {
    #[serde(rename = "spice")]
    #[default]
    /// spice.
    Spice,
    #[serde(rename = "none")]
    /// none.
    None,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigAudio0Driver);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigAudio0Driver);

#[api]
/// Select BIOS implementation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigBios {
    #[serde(rename = "seabios")]
    #[default]
    /// seabios.
    Seabios,
    #[serde(rename = "ovmf")]
    /// ovmf.
    Ovmf,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigBios);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigBios);

#[api]
/// Specifies the cloud-init configuration format. The default depends on the
/// configured operating system type (`ostype`. We use the `nocloud` format for
/// Linux, and `configdrive2` for windows.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigCitype {
    #[serde(rename = "configdrive2")]
    /// configdrive2.
    Configdrive2,
    #[serde(rename = "nocloud")]
    /// nocloud.
    Nocloud,
    #[serde(rename = "opennebula")]
    /// opennebula.
    Opennebula,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigCitype);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigCitype);

const_regex! {

QEMU_CONFIG_EFIDISK0_SIZE_RE = r##"^(\d+(\.\d+)?)([KMGT])?$"##;

}

#[test]
fn test_regex_compilation_18() {
    use regex::Regex;
    let _: &Regex = &QEMU_CONFIG_EFIDISK0_SIZE_RE;
}
#[api(
    default_key: "file",
    properties: {
        efitype: {
            optional: true,
            type: QemuConfigEfidisk0Efitype,
        },
        file: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_volume_id_or_qm_path),
            type: String,
        },
        format: {
            optional: true,
            type: PveQmIdeFormat,
        },
        "pre-enrolled-keys": {
            default: false,
            optional: true,
        },
        size: {
            format: &ApiStringFormat::Pattern(&QEMU_CONFIG_EFIDISK0_SIZE_RE),
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfigEfidisk0 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub efitype: Option<QemuConfigEfidisk0Efitype>,

    /// The drive's backing volume.
    pub file: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<PveQmIdeFormat>,

    /// Use am EFI vars template with distribution-specific and Microsoft
    /// Standard keys enrolled, if used with 'efitype=4m'. Note that this will
    /// enable Secure Boot by default, though it can still be turned off from
    /// within the VM.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "pre-enrolled-keys")]
    pub pre_enrolled_keys: Option<bool>,

    /// Disk size. This is purely informational and has no effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
}

#[api]
/// Size and type of the OVMF EFI vars. '4m' is newer and recommended, and
/// required for Secure Boot. For backwards compatibility, '2m' is used if not
/// otherwise specified. Ignored for VMs with arch=aarch64 (ARM).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigEfidisk0Efitype {
    #[serde(rename = "2m")]
    #[default]
    /// 2m.
    Mb2,
    #[serde(rename = "4m")]
    /// 4m.
    Mb4,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigEfidisk0Efitype);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigEfidisk0Efitype);

#[api]
/// Enable/disable hugepages memory.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigHugepages {
    #[serde(rename = "any")]
    /// any.
    Any,
    #[serde(rename = "2")]
    /// 2.
    Mb2,
    #[serde(rename = "1024")]
    /// 1024.
    Mb1024,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigHugepages);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigHugepages);

#[api(
    properties: {
        name: {
            optional: true,
            type: String,
        },
        size: {
            minimum: 1,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfigIvshmem {
    /// The name of the file. Will be prefixed with 'pve-shm-'. Default is the
    /// VMID. Will be deleted when the VM is stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// The size of the file in MB.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    pub size: u64,
}

#[api]
/// Keyboard layout for VNC server. This option is generally not required and is
/// often better handled from within the guest OS.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigKeyboard {
    #[serde(rename = "de")]
    /// de.
    De,
    #[serde(rename = "de-ch")]
    /// de-ch.
    DeCh,
    #[serde(rename = "da")]
    /// da.
    Da,
    #[serde(rename = "en-gb")]
    /// en-gb.
    EnGb,
    #[serde(rename = "en-us")]
    /// en-us.
    EnUs,
    #[serde(rename = "es")]
    /// es.
    Es,
    #[serde(rename = "fi")]
    /// fi.
    Fi,
    #[serde(rename = "fr")]
    /// fr.
    Fr,
    #[serde(rename = "fr-be")]
    /// fr-be.
    FrBe,
    #[serde(rename = "fr-ca")]
    /// fr-ca.
    FrCa,
    #[serde(rename = "fr-ch")]
    /// fr-ch.
    FrCh,
    #[serde(rename = "hu")]
    /// hu.
    Hu,
    #[serde(rename = "is")]
    /// is.
    Is,
    #[serde(rename = "it")]
    /// it.
    It,
    #[serde(rename = "ja")]
    /// ja.
    Ja,
    #[serde(rename = "lt")]
    /// lt.
    Lt,
    #[serde(rename = "mk")]
    /// mk.
    Mk,
    #[serde(rename = "nl")]
    /// nl.
    Nl,
    #[serde(rename = "no")]
    /// no.
    No,
    #[serde(rename = "pl")]
    /// pl.
    Pl,
    #[serde(rename = "pt")]
    /// pt.
    Pt,
    #[serde(rename = "pt-br")]
    /// pt-br.
    PtBr,
    #[serde(rename = "sv")]
    /// sv.
    Sv,
    #[serde(rename = "sl")]
    /// sl.
    Sl,
    #[serde(rename = "tr")]
    /// tr.
    Tr,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigKeyboard);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigKeyboard);

#[api]
/// Lock/unlock the VM.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigLock {
    #[serde(rename = "backup")]
    /// backup.
    Backup,
    #[serde(rename = "clone")]
    /// clone.
    Clone,
    #[serde(rename = "create")]
    /// create.
    Create,
    #[serde(rename = "migrate")]
    /// migrate.
    Migrate,
    #[serde(rename = "rollback")]
    /// rollback.
    Rollback,
    #[serde(rename = "snapshot")]
    /// snapshot.
    Snapshot,
    #[serde(rename = "snapshot-delete")]
    /// snapshot-delete.
    SnapshotDelete,
    #[serde(rename = "suspending")]
    /// suspending.
    Suspending,
    #[serde(rename = "suspended")]
    /// suspended.
    Suspended,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigLock);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigLock);

#[api(
    default_key: "type",
    properties: {
        "aw-bits": {
            maximum: 64.0,
            minimum: 32.0,
            optional: true,
        },
        "enable-s3": {
            default: false,
            optional: true,
        },
        "enable-s4": {
            default: false,
            optional: true,
        },
        type: {
            max_length: 40,
            optional: true,
            type: String,
        },
        viommu: {
            optional: true,
            type: QemuConfigMachineViommu,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfigMachine {
    /// Specifies the vIOMMU address space bit width.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "aw-bits")]
    pub aw_bits: Option<f64>,

    /// Enables S3 power state. Defaults to false beginning with machine types
    /// 9.2+pve1, true before.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "enable-s3")]
    pub enable_s3: Option<bool>,

    /// Enables S4 power state. Defaults to false beginning with machine types
    /// 9.2+pve1, true before.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "enable-s4")]
    pub enable_s4: Option<bool>,

    /// Specifies the QEMU machine type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub ty: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viommu: Option<QemuConfigMachineViommu>,
}

#[api]
/// Enable and set guest vIOMMU variant (Intel vIOMMU needs q35 to be set as
/// machine type).
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigMachineViommu {
    #[serde(rename = "intel")]
    /// intel.
    Intel,
    #[serde(rename = "virtio")]
    /// virtio.
    Virtio,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigMachineViommu);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigMachineViommu);

#[api(
    default_key: "current",
    properties: {
        current: {
            default: 512,
            minimum: 16,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfigMemory {
    /// Current amount of online RAM for the VM in MiB. This is the maximum
    /// available memory when you use the balloon device.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    pub current: u64,
}

#[api(
    properties: {
        "creation-qemu": {
            optional: true,
            type: String,
        },
        ctime: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfigMeta {
    /// The QEMU (machine) version from the time this VM was created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "creation-qemu")]
    pub creation_qemu: Option<String>,

    /// The guest creation timestamp as UNIX epoch time
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctime: Option<u64>,
}

const_regex! {

QEMU_CONFIG_NET_BRIDGE_RE = r##"^[-_.\w\d]+$"##;
QEMU_CONFIG_NET_MACADDR_RE = r##"^(?i)[a-f0-9][02468ace](?::[a-f0-9]{2}){5}$"##;

}

#[test]
fn test_regex_compilation_19() {
    use regex::Regex;
    let _: &Regex = &QEMU_CONFIG_NET_BRIDGE_RE;
    let _: &Regex = &QEMU_CONFIG_NET_MACADDR_RE;
}
#[api(
    default_key: "model",
    key_alias_info: proxmox_schema::KeyAliasInfo::new(
        "model",
        &[
            "e1000",
            "e1000-82540em",
            "e1000-82544gc",
            "e1000-82545em",
            "e1000e",
            "i82551",
            "i82557b",
            "i82559er",
            "ne2k_isa",
            "ne2k_pci",
            "pcnet",
            "rtl8139",
            "virtio",
            "vmxnet3"
        ],
        "macaddr"
    ),
    properties: {
        bridge: {
            format: &ApiStringFormat::Pattern(&QEMU_CONFIG_NET_BRIDGE_RE),
            optional: true,
            type: String,
        },
        firewall: {
            default: false,
            optional: true,
        },
        link_down: {
            default: false,
            optional: true,
        },
        macaddr: {
            format: &ApiStringFormat::Pattern(&QEMU_CONFIG_NET_MACADDR_RE),
            optional: true,
            type: String,
        },
        model: {
            type: QemuConfigNetModel,
        },
        mtu: {
            maximum: 65520,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        queues: {
            maximum: 64,
            minimum: 0,
            optional: true,
            type: Integer,
        },
        rate: {
            minimum: 0.0,
            optional: true,
        },
        tag: {
            maximum: 4094,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        trunks: {
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfigNet {
    /// Bridge to attach the network device to. The Proxmox VE standard bridge
    /// is called 'vmbr0'.
    ///
    /// If you do not specify a bridge, we create a kvm user (NATed) network
    /// device, which provides DHCP and DNS services. The following addresses
    /// are used:
    ///
    ///  10.0.2.2   Gateway
    ///  10.0.2.3   DNS Server
    ///  10.0.2.4   SMB Server
    ///
    /// The DHCP server assign addresses to the guest starting from 10.0.2.15.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge: Option<String>,

    /// Whether this interface should be protected by the firewall.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firewall: Option<bool>,

    /// Whether this interface should be disconnected (like pulling the plug).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_down: Option<bool>,

    /// MAC address. That address must be unique within your network. This is
    /// automatically generated if not specified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macaddr: Option<String>,

    pub model: QemuConfigNetModel,

    /// Force MTU of network device (VirtIO only). Setting to '1' or empty will
    /// use the bridge MTU
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u16")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtu: Option<u16>,

    /// Number of packet queues to be used on the device.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u8")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queues: Option<u8>,

    /// Rate limit in mbps (megabytes per second) as floating point number.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<f64>,

    /// VLAN tag to apply to packets on this interface.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u16")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<u16>,

    /// VLAN trunks to pass through this interface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trunks: Option<String>,
}

#[api]
/// Network Card Model. The 'virtio' model provides the best performance with
/// very low CPU overhead. If your guest does not support this driver, it is
/// usually best to use 'e1000'.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigNetModel {
    #[serde(rename = "e1000")]
    /// e1000.
    E1000,
    #[serde(rename = "e1000-82540em")]
    /// e1000-82540em.
    E100082540em,
    #[serde(rename = "e1000-82544gc")]
    /// e1000-82544gc.
    E100082544gc,
    #[serde(rename = "e1000-82545em")]
    /// e1000-82545em.
    E100082545em,
    #[serde(rename = "e1000e")]
    /// e1000e.
    E1000e,
    #[serde(rename = "i82551")]
    /// i82551.
    I82551,
    #[serde(rename = "i82557b")]
    /// i82557b.
    I82557b,
    #[serde(rename = "i82559er")]
    /// i82559er.
    I82559er,
    #[serde(rename = "ne2k_isa")]
    /// ne2k_isa.
    Ne2kIsa,
    #[serde(rename = "ne2k_pci")]
    /// ne2k_pci.
    Ne2kPci,
    #[serde(rename = "pcnet")]
    /// pcnet.
    Pcnet,
    #[serde(rename = "rtl8139")]
    /// rtl8139.
    Rtl8139,
    #[serde(rename = "virtio")]
    /// virtio.
    Virtio,
    #[serde(rename = "vmxnet3")]
    /// vmxnet3.
    Vmxnet3,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigNetModel);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigNetModel);

#[api(
    properties: {
        cpus: {
            type: String,
        },
        hostnodes: {
            optional: true,
            type: String,
        },
        policy: {
            optional: true,
            type: QemuConfigNumaPolicy,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfigNuma {
    /// CPUs accessing this NUMA node.
    pub cpus: String,

    /// Host NUMA nodes to use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostnodes: Option<String>,

    /// Amount of memory this NUMA node provides.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<QemuConfigNumaPolicy>,
}

#[api]
/// NUMA allocation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigNumaPolicy {
    #[serde(rename = "preferred")]
    /// preferred.
    Preferred,
    #[serde(rename = "bind")]
    /// bind.
    Bind,
    #[serde(rename = "interleave")]
    /// interleave.
    Interleave,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigNumaPolicy);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigNumaPolicy);

#[api]
/// Specify guest operating system.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigOstype {
    #[serde(rename = "other")]
    #[default]
    /// other.
    Other,
    #[serde(rename = "wxp")]
    /// wxp.
    Wxp,
    #[serde(rename = "w2k")]
    /// w2k.
    W2k,
    #[serde(rename = "w2k3")]
    /// w2k3.
    W2k3,
    #[serde(rename = "w2k8")]
    /// w2k8.
    W2k8,
    #[serde(rename = "wvista")]
    /// wvista.
    Wvista,
    #[serde(rename = "win7")]
    /// win7.
    Win7,
    #[serde(rename = "win8")]
    /// win8.
    Win8,
    #[serde(rename = "win10")]
    /// win10.
    Win10,
    #[serde(rename = "win11")]
    /// win11.
    Win11,
    #[serde(rename = "l24")]
    /// l24.
    L24,
    #[serde(rename = "l26")]
    /// l26.
    L26,
    #[serde(rename = "solaris")]
    /// solaris.
    Solaris,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigOstype);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigOstype);

const_regex! {

QEMU_CONFIG_SATA_SERIAL_RE = r##"^[-%a-zA-Z0-9_.!~*'()]*$"##;
QEMU_CONFIG_SATA_SIZE_RE = r##"^(\d+(\.\d+)?)([KMGT])?$"##;

}

#[test]
fn test_regex_compilation_20() {
    use regex::Regex;
    let _: &Regex = &QEMU_CONFIG_SATA_SERIAL_RE;
    let _: &Regex = &QEMU_CONFIG_SATA_SIZE_RE;
}
#[api(
    default_key: "file",
    properties: {
        aio: {
            optional: true,
            type: PveQmIdeAio,
        },
        backup: {
            default: false,
            optional: true,
        },
        bps: {
            optional: true,
            type: Integer,
        },
        bps_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        bps_rd: {
            optional: true,
            type: Integer,
        },
        bps_rd_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        bps_wr: {
            optional: true,
            type: Integer,
        },
        bps_wr_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        cache: {
            optional: true,
            type: PveQmIdeCache,
        },
        detect_zeroes: {
            default: false,
            optional: true,
        },
        discard: {
            optional: true,
            type: PveQmIdeDiscard,
        },
        file: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_volume_id_or_qm_path),
            type: String,
        },
        format: {
            optional: true,
            type: PveQmIdeFormat,
        },
        iops: {
            optional: true,
            type: Integer,
        },
        iops_max: {
            optional: true,
            type: Integer,
        },
        iops_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iops_rd: {
            optional: true,
            type: Integer,
        },
        iops_rd_max: {
            optional: true,
            type: Integer,
        },
        iops_rd_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iops_wr: {
            optional: true,
            type: Integer,
        },
        iops_wr_max: {
            optional: true,
            type: Integer,
        },
        iops_wr_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        media: {
            optional: true,
            type: PveQmIdeMedia,
        },
        replicate: {
            default: true,
            optional: true,
        },
        rerror: {
            optional: true,
            type: PveQmIdeRerror,
        },
        serial: {
            format: &ApiStringFormat::Pattern(&QEMU_CONFIG_SATA_SERIAL_RE),
            max_length: 60,
            optional: true,
            type: String,
        },
        shared: {
            default: false,
            optional: true,
        },
        size: {
            format: &ApiStringFormat::Pattern(&QEMU_CONFIG_SATA_SIZE_RE),
            optional: true,
            type: String,
        },
        snapshot: {
            default: false,
            optional: true,
        },
        ssd: {
            default: false,
            optional: true,
        },
        werror: {
            optional: true,
            type: PveQmIdeWerror,
        },
        wwn: {
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfigSata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aio: Option<PveQmIdeAio>,

    /// Whether the drive should be included when making backups.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<bool>,

    /// Maximum r/w speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps: Option<i64>,

    /// Maximum length of I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_max_length: Option<u64>,

    /// Maximum read speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_rd: Option<i64>,

    /// Maximum length of read I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_rd_max_length: Option<u64>,

    /// Maximum write speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_wr: Option<i64>,

    /// Maximum length of write I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_wr_max_length: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<PveQmIdeCache>,

    /// Controls whether to detect and try to optimize writes of zeroes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect_zeroes: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discard: Option<PveQmIdeDiscard>,

    /// The drive's backing volume.
    pub file: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<PveQmIdeFormat>,

    /// Maximum r/w I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops: Option<i64>,

    /// Maximum unthrottled r/w I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_max: Option<i64>,

    /// Maximum length of I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_max_length: Option<u64>,

    /// Maximum read I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd: Option<i64>,

    /// Maximum unthrottled read I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd_max: Option<i64>,

    /// Maximum length of read I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd_max_length: Option<u64>,

    /// Maximum write I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr: Option<i64>,

    /// Maximum unthrottled write I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr_max: Option<i64>,

    /// Maximum length of write I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr_max_length: Option<u64>,

    /// Maximum r/w speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps: Option<f64>,

    /// Maximum unthrottled r/w pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_max: Option<f64>,

    /// Maximum read speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_rd: Option<f64>,

    /// Maximum unthrottled read pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_rd_max: Option<f64>,

    /// Maximum write speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_wr: Option<f64>,

    /// Maximum unthrottled write pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_wr_max: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<PveQmIdeMedia>,

    /// Whether the drive should considered for replication jobs.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicate: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerror: Option<PveQmIdeRerror>,

    /// The drive's reported serial number, url-encoded, up to 20 bytes long.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,

    /// Mark this locally-managed volume as available on all nodes
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared: Option<bool>,

    /// Disk size. This is purely informational and has no effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    /// Controls qemu's snapshot mode feature. If activated, changes made to the
    /// disk are temporary and will be discarded when the VM is shutdown.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<bool>,

    /// Whether to expose this drive as an SSD, rather than a rotational hard
    /// disk.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssd: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub werror: Option<PveQmIdeWerror>,

    /// The drive's worldwide name, encoded as 16 bytes hex string, prefixed by
    /// '0x'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wwn: Option<String>,
}

const_regex! {

QEMU_CONFIG_SCSI_SERIAL_RE = r##"^[-%a-zA-Z0-9_.!~*'()]*$"##;
QEMU_CONFIG_SCSI_SIZE_RE = r##"^(\d+(\.\d+)?)([KMGT])?$"##;

}

#[test]
fn test_regex_compilation_21() {
    use regex::Regex;
    let _: &Regex = &QEMU_CONFIG_SCSI_SERIAL_RE;
    let _: &Regex = &QEMU_CONFIG_SCSI_SIZE_RE;
}
#[api(
    default_key: "file",
    properties: {
        aio: {
            optional: true,
            type: PveQmIdeAio,
        },
        backup: {
            default: false,
            optional: true,
        },
        bps: {
            optional: true,
            type: Integer,
        },
        bps_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        bps_rd: {
            optional: true,
            type: Integer,
        },
        bps_rd_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        bps_wr: {
            optional: true,
            type: Integer,
        },
        bps_wr_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        cache: {
            optional: true,
            type: PveQmIdeCache,
        },
        detect_zeroes: {
            default: false,
            optional: true,
        },
        discard: {
            optional: true,
            type: PveQmIdeDiscard,
        },
        file: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_volume_id_or_qm_path),
            type: String,
        },
        format: {
            optional: true,
            type: PveQmIdeFormat,
        },
        iops: {
            optional: true,
            type: Integer,
        },
        iops_max: {
            optional: true,
            type: Integer,
        },
        iops_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iops_rd: {
            optional: true,
            type: Integer,
        },
        iops_rd_max: {
            optional: true,
            type: Integer,
        },
        iops_rd_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iops_wr: {
            optional: true,
            type: Integer,
        },
        iops_wr_max: {
            optional: true,
            type: Integer,
        },
        iops_wr_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iothread: {
            default: false,
            optional: true,
        },
        media: {
            optional: true,
            type: PveQmIdeMedia,
        },
        product: {
            optional: true,
            type: String,
        },
        queues: {
            minimum: 2,
            optional: true,
            type: Integer,
        },
        replicate: {
            default: true,
            optional: true,
        },
        rerror: {
            optional: true,
            type: PveQmIdeRerror,
        },
        ro: {
            default: false,
            optional: true,
        },
        scsiblock: {
            default: false,
            optional: true,
        },
        serial: {
            format: &ApiStringFormat::Pattern(&QEMU_CONFIG_SCSI_SERIAL_RE),
            max_length: 60,
            optional: true,
            type: String,
        },
        shared: {
            default: false,
            optional: true,
        },
        size: {
            format: &ApiStringFormat::Pattern(&QEMU_CONFIG_SCSI_SIZE_RE),
            optional: true,
            type: String,
        },
        snapshot: {
            default: false,
            optional: true,
        },
        ssd: {
            default: false,
            optional: true,
        },
        vendor: {
            optional: true,
            type: String,
        },
        werror: {
            optional: true,
            type: PveQmIdeWerror,
        },
        wwn: {
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfigScsi {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aio: Option<PveQmIdeAio>,

    /// Whether the drive should be included when making backups.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<bool>,

    /// Maximum r/w speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps: Option<i64>,

    /// Maximum length of I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_max_length: Option<u64>,

    /// Maximum read speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_rd: Option<i64>,

    /// Maximum length of read I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_rd_max_length: Option<u64>,

    /// Maximum write speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_wr: Option<i64>,

    /// Maximum length of write I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_wr_max_length: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<PveQmIdeCache>,

    /// Controls whether to detect and try to optimize writes of zeroes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect_zeroes: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discard: Option<PveQmIdeDiscard>,

    /// The drive's backing volume.
    pub file: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<PveQmIdeFormat>,

    /// Maximum r/w I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops: Option<i64>,

    /// Maximum unthrottled r/w I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_max: Option<i64>,

    /// Maximum length of I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_max_length: Option<u64>,

    /// Maximum read I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd: Option<i64>,

    /// Maximum unthrottled read I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd_max: Option<i64>,

    /// Maximum length of read I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd_max_length: Option<u64>,

    /// Maximum write I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr: Option<i64>,

    /// Maximum unthrottled write I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr_max: Option<i64>,

    /// Maximum length of write I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr_max_length: Option<u64>,

    /// Whether to use iothreads for this drive
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iothread: Option<bool>,

    /// Maximum r/w speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps: Option<f64>,

    /// Maximum unthrottled r/w pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_max: Option<f64>,

    /// Maximum read speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_rd: Option<f64>,

    /// Maximum unthrottled read pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_rd_max: Option<f64>,

    /// Maximum write speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_wr: Option<f64>,

    /// Maximum unthrottled write pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_wr_max: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<PveQmIdeMedia>,

    /// The drive's product name, up to 16 bytes long.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,

    /// Number of queues.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queues: Option<u64>,

    /// Whether the drive should considered for replication jobs.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicate: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerror: Option<PveQmIdeRerror>,

    /// Whether the drive is read-only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ro: Option<bool>,

    /// whether to use scsi-block for full passthrough of host block device
    ///
    /// WARNING: can lead to I/O errors in combination with low memory or high
    /// memory fragmentation on host
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scsiblock: Option<bool>,

    /// The drive's reported serial number, url-encoded, up to 20 bytes long.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,

    /// Mark this locally-managed volume as available on all nodes
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared: Option<bool>,

    /// Disk size. This is purely informational and has no effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    /// Controls qemu's snapshot mode feature. If activated, changes made to the
    /// disk are temporary and will be discarded when the VM is shutdown.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<bool>,

    /// Whether to expose this drive as an SSD, rather than a rotational hard
    /// disk.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssd: Option<bool>,

    /// The drive's vendor name, up to 8 bytes long.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub werror: Option<PveQmIdeWerror>,

    /// The drive's worldwide name, encoded as 16 bytes hex string, prefixed by
    /// '0x'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wwn: Option<String>,
}

#[api]
/// SCSI controller model
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigScsihw {
    #[serde(rename = "lsi")]
    #[default]
    /// lsi.
    Lsi,
    #[serde(rename = "lsi53c810")]
    /// lsi53c810.
    Lsi53c810,
    #[serde(rename = "virtio-scsi-pci")]
    /// virtio-scsi-pci.
    VirtioScsiPci,
    #[serde(rename = "virtio-scsi-single")]
    /// virtio-scsi-single.
    VirtioScsiSingle,
    #[serde(rename = "megasas")]
    /// megasas.
    Megasas,
    #[serde(rename = "pvscsi")]
    /// pvscsi.
    Pvscsi,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigScsihw);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigScsihw);

#[api(
    properties: {
        foldersharing: {
            default: false,
            optional: true,
        },
        videostreaming: {
            optional: true,
            type: QemuConfigSpiceEnhancementsVideostreaming,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfigSpiceEnhancements {
    /// Enable folder sharing via SPICE. Needs Spice-WebDAV daemon installed in
    /// the VM.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foldersharing: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub videostreaming: Option<QemuConfigSpiceEnhancementsVideostreaming>,
}

#[api]
/// Enable video streaming. Uses compression for detected video streams.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigSpiceEnhancementsVideostreaming {
    #[serde(rename = "off")]
    #[default]
    /// off.
    Off,
    #[serde(rename = "all")]
    /// all.
    All,
    #[serde(rename = "filter")]
    /// filter.
    Filter,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigSpiceEnhancementsVideostreaming);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigSpiceEnhancementsVideostreaming);

const_regex! {

QEMU_CONFIG_TPMSTATE0_SIZE_RE = r##"^(\d+(\.\d+)?)([KMGT])?$"##;

}

#[test]
fn test_regex_compilation_22() {
    use regex::Regex;
    let _: &Regex = &QEMU_CONFIG_TPMSTATE0_SIZE_RE;
}
#[api(
    default_key: "file",
    properties: {
        file: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_volume_id_or_qm_path),
            type: String,
        },
        size: {
            format: &ApiStringFormat::Pattern(&QEMU_CONFIG_TPMSTATE0_SIZE_RE),
            optional: true,
            type: String,
        },
        version: {
            optional: true,
            type: QemuConfigTpmstate0Version,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfigTpmstate0 {
    /// The drive's backing volume.
    pub file: String,

    /// Disk size. This is purely informational and has no effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<QemuConfigTpmstate0Version>,
}

#[api]
/// The TPM interface version. v2.0 is newer and should be preferred. Note that
/// this cannot be changed later on.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigTpmstate0Version {
    #[serde(rename = "v1.2")]
    #[default]
    /// v1.2.
    V1_2,
    #[serde(rename = "v2.0")]
    /// v2.0.
    V2_0,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigTpmstate0Version);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigTpmstate0Version);

#[api(
    default_key: "file",
    properties: {
        file: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_volume_id),
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfigUnused {
    /// The drive's backing volume.
    pub file: String,
}

const_regex! {

QEMU_CONFIG_USB_MAPPING_RE = r##"^(?i:[a-z][a-z0-9_-]+)$"##;

}

#[test]
fn test_regex_compilation_23() {
    use regex::Regex;
    let _: &Regex = &QEMU_CONFIG_USB_MAPPING_RE;
}
#[api(
    default_key: "host",
    properties: {
        host: {
            optional: true,
            type: String,
        },
        mapping: {
            format: &ApiStringFormat::Pattern(&QEMU_CONFIG_USB_MAPPING_RE),
            optional: true,
            type: String,
        },
        usb3: {
            default: false,
            optional: true,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfigUsb {
    /// The Host USB device or port or the value 'spice'. HOSTUSBDEVICE syntax
    /// is:
    ///
    ///  'bus-port(.port)*' (decimal numbers) or
    ///  'vendor_id:product_id' (hexadecimal numbers) or
    ///  'spice'
    ///
    /// You can use the 'lsusb -t' command to list existing usb devices.
    ///
    /// NOTE: This option allows direct access to host hardware. So it is no
    /// longer possible to migrate such machines - use with special care.
    ///
    /// The value 'spice' can be used to add a usb redirection devices for
    /// spice.
    ///
    /// Either this or the 'mapping' key must be set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,

    /// The ID of a cluster wide mapping. Either this or the default-key 'host'
    /// must be set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mapping: Option<String>,

    /// Specifies whether if given host option is a USB3 device or port. For
    /// modern guests (machine version >= 7.1 and ostype l26 and windows > 7),
    /// this flag is irrelevant (all devices are plugged into a xhci
    /// controller).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usb3: Option<bool>,
}

#[api(
    default_key: "type",
    properties: {
        clipboard: {
            optional: true,
            type: QemuConfigVgaClipboard,
        },
        memory: {
            maximum: 512,
            minimum: 4,
            optional: true,
            type: Integer,
        },
        type: {
            optional: true,
            type: QemuConfigVgaType,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfigVga {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clipboard: Option<QemuConfigVgaClipboard>,

    /// Sets the VGA memory (in MiB). Has no effect with serial display.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u16")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<u16>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub ty: Option<QemuConfigVgaType>,
}

#[api]
/// Enable a specific clipboard. If not set, depending on the display type the
/// SPICE one will be added. Migration with VNC clipboard is not yet supported!
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigVgaClipboard {
    #[serde(rename = "vnc")]
    /// vnc.
    Vnc,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigVgaClipboard);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigVgaClipboard);

#[api]
/// Select the VGA type. Using type 'cirrus' is not recommended.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigVgaType {
    #[serde(rename = "cirrus")]
    /// cirrus.
    Cirrus,
    #[serde(rename = "qxl")]
    /// qxl.
    Qxl,
    #[serde(rename = "qxl2")]
    /// qxl2.
    Qxl2,
    #[serde(rename = "qxl3")]
    /// qxl3.
    Qxl3,
    #[serde(rename = "qxl4")]
    /// qxl4.
    Qxl4,
    #[serde(rename = "none")]
    /// none.
    None,
    #[serde(rename = "serial0")]
    /// serial0.
    Serial0,
    #[serde(rename = "serial1")]
    /// serial1.
    Serial1,
    #[serde(rename = "serial2")]
    /// serial2.
    Serial2,
    #[serde(rename = "serial3")]
    /// serial3.
    Serial3,
    #[serde(rename = "std")]
    #[default]
    /// std.
    Std,
    #[serde(rename = "virtio")]
    /// virtio.
    Virtio,
    #[serde(rename = "virtio-gl")]
    /// virtio-gl.
    VirtioGl,
    #[serde(rename = "vmware")]
    /// vmware.
    Vmware,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigVgaType);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigVgaType);

const_regex! {

QEMU_CONFIG_VIRTIO_SERIAL_RE = r##"^[-%a-zA-Z0-9_.!~*'()]*$"##;
QEMU_CONFIG_VIRTIO_SIZE_RE = r##"^(\d+(\.\d+)?)([KMGT])?$"##;

}

#[test]
fn test_regex_compilation_24() {
    use regex::Regex;
    let _: &Regex = &QEMU_CONFIG_VIRTIO_SERIAL_RE;
    let _: &Regex = &QEMU_CONFIG_VIRTIO_SIZE_RE;
}
#[api(
    default_key: "file",
    properties: {
        aio: {
            optional: true,
            type: PveQmIdeAio,
        },
        backup: {
            default: false,
            optional: true,
        },
        bps: {
            optional: true,
            type: Integer,
        },
        bps_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        bps_rd: {
            optional: true,
            type: Integer,
        },
        bps_rd_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        bps_wr: {
            optional: true,
            type: Integer,
        },
        bps_wr_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        cache: {
            optional: true,
            type: PveQmIdeCache,
        },
        detect_zeroes: {
            default: false,
            optional: true,
        },
        discard: {
            optional: true,
            type: PveQmIdeDiscard,
        },
        file: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_volume_id_or_qm_path),
            type: String,
        },
        format: {
            optional: true,
            type: PveQmIdeFormat,
        },
        iops: {
            optional: true,
            type: Integer,
        },
        iops_max: {
            optional: true,
            type: Integer,
        },
        iops_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iops_rd: {
            optional: true,
            type: Integer,
        },
        iops_rd_max: {
            optional: true,
            type: Integer,
        },
        iops_rd_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iops_wr: {
            optional: true,
            type: Integer,
        },
        iops_wr_max: {
            optional: true,
            type: Integer,
        },
        iops_wr_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iothread: {
            default: false,
            optional: true,
        },
        media: {
            optional: true,
            type: PveQmIdeMedia,
        },
        replicate: {
            default: true,
            optional: true,
        },
        rerror: {
            optional: true,
            type: PveQmIdeRerror,
        },
        ro: {
            default: false,
            optional: true,
        },
        serial: {
            format: &ApiStringFormat::Pattern(&QEMU_CONFIG_VIRTIO_SERIAL_RE),
            max_length: 60,
            optional: true,
            type: String,
        },
        shared: {
            default: false,
            optional: true,
        },
        size: {
            format: &ApiStringFormat::Pattern(&QEMU_CONFIG_VIRTIO_SIZE_RE),
            optional: true,
            type: String,
        },
        snapshot: {
            default: false,
            optional: true,
        },
        werror: {
            optional: true,
            type: PveQmIdeWerror,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfigVirtio {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aio: Option<PveQmIdeAio>,

    /// Whether the drive should be included when making backups.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<bool>,

    /// Maximum r/w speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps: Option<i64>,

    /// Maximum length of I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_max_length: Option<u64>,

    /// Maximum read speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_rd: Option<i64>,

    /// Maximum length of read I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_rd_max_length: Option<u64>,

    /// Maximum write speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_wr: Option<i64>,

    /// Maximum length of write I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_wr_max_length: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<PveQmIdeCache>,

    /// Controls whether to detect and try to optimize writes of zeroes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect_zeroes: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discard: Option<PveQmIdeDiscard>,

    /// The drive's backing volume.
    pub file: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<PveQmIdeFormat>,

    /// Maximum r/w I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops: Option<i64>,

    /// Maximum unthrottled r/w I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_max: Option<i64>,

    /// Maximum length of I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_max_length: Option<u64>,

    /// Maximum read I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd: Option<i64>,

    /// Maximum unthrottled read I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd_max: Option<i64>,

    /// Maximum length of read I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd_max_length: Option<u64>,

    /// Maximum write I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr: Option<i64>,

    /// Maximum unthrottled write I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr_max: Option<i64>,

    /// Maximum length of write I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr_max_length: Option<u64>,

    /// Whether to use iothreads for this drive
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iothread: Option<bool>,

    /// Maximum r/w speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps: Option<f64>,

    /// Maximum unthrottled r/w pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_max: Option<f64>,

    /// Maximum read speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_rd: Option<f64>,

    /// Maximum unthrottled read pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_rd_max: Option<f64>,

    /// Maximum write speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_wr: Option<f64>,

    /// Maximum unthrottled write pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_wr_max: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<PveQmIdeMedia>,

    /// Whether the drive should considered for replication jobs.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicate: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerror: Option<PveQmIdeRerror>,

    /// Whether the drive is read-only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ro: Option<bool>,

    /// The drive's reported serial number, url-encoded, up to 20 bytes long.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,

    /// Mark this locally-managed volume as available on all nodes
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared: Option<bool>,

    /// Disk size. This is purely informational and has no effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    /// Controls qemu's snapshot mode feature. If activated, changes made to the
    /// disk are temporary and will be discarded when the VM is shutdown.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub werror: Option<PveQmIdeWerror>,
}

const_regex! {

QEMU_CONFIG_VIRTIOFS_DIRID_RE = r##"^(?i:[a-z][a-z0-9_-]+)$"##;

}

#[test]
fn test_regex_compilation_25() {
    use regex::Regex;
    let _: &Regex = &QEMU_CONFIG_VIRTIOFS_DIRID_RE;
}
#[api(
    default_key: "dirid",
    properties: {
        cache: {
            optional: true,
            type: QemuConfigVirtiofsCache,
        },
        "direct-io": {
            default: false,
            optional: true,
        },
        dirid: {
            format: &ApiStringFormat::Pattern(&QEMU_CONFIG_VIRTIOFS_DIRID_RE),
            type: String,
        },
        "expose-acl": {
            default: false,
            optional: true,
        },
        "expose-xattr": {
            default: false,
            optional: true,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuConfigVirtiofs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<QemuConfigVirtiofsCache>,

    /// Honor the O_DIRECT flag passed down by guest applications.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "direct-io")]
    pub direct_io: Option<bool>,

    /// Mapping identifier of the directory mapping to be shared with the guest.
    /// Also used as a mount tag inside the VM.
    pub dirid: String,

    /// Enable support for POSIX ACLs (enabled ACL implies xattr) for this
    /// mount.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "expose-acl")]
    pub expose_acl: Option<bool>,

    /// Enable support for extended attributes for this mount.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "expose-xattr")]
    pub expose_xattr: Option<bool>,
}

#[api]
/// The caching policy the file system should use (auto, always, metadata,
/// never).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuConfigVirtiofsCache {
    #[serde(rename = "auto")]
    #[default]
    /// auto.
    Auto,
    #[serde(rename = "always")]
    /// always.
    Always,
    #[serde(rename = "metadata")]
    /// metadata.
    Metadata,
    #[serde(rename = "never")]
    /// never.
    Never,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuConfigVirtiofsCache);
serde_plain::derive_fromstr_from_deserialize!(QemuConfigVirtiofsCache);

#[api(
    properties: {
        allowed_nodes: {
            items: {
                description: "An allowed node",
                type: String,
            },
            optional: true,
            type: Array,
        },
        "dependent-ha-resources": {
            items: {
                description: "The '<ty>:<id>' resource IDs of a HA resource with a positive affinity rule to this VM.",
                type: String,
            },
            optional: true,
            type: Array,
        },
        "has-dbus-vmstate": {
            default: false,
        },
        local_disks: {
            items: {
                type: QemuMigratePreconditionsLocalDisks,
            },
            type: Array,
        },
        local_resources: {
            items: {
                description: "A local resource",
                type: String,
            },
            type: Array,
        },
        "mapped-resource-info": {
            description: "Object of mapped resources with additional information such if they're live migratable.",
            properties: {},
            type: Object,
        },
        "mapped-resources": {
            items: {
                description: "A mapped resource",
                type: String,
            },
            type: Array,
        },
        not_allowed_nodes: {
            optional: true,
            type: QemuMigratePreconditionsNotAllowedNodes,
        },
        running: {
            default: false,
        },
    },
)]
/// Object.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct QemuMigratePreconditions {
    /// List of nodes allowed for migration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_nodes: Option<Vec<String>>,

    /// HA resources, which will be migrated to the same target node as the VM,
    /// because these are in positive affinity with the VM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "dependent-ha-resources")]
    pub dependent_ha_resources: Option<Vec<String>>,

    /// Whether the VM host supports migrating additional VM state, such as
    /// conntrack entries.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(rename = "has-dbus-vmstate")]
    pub has_dbus_vmstate: bool,

    /// List local disks including CD-Rom, unused and not referenced disks
    pub local_disks: Vec<QemuMigratePreconditionsLocalDisks>,

    /// List local resources (e.g. pci, usb) that block migration.
    pub local_resources: Vec<String>,

    /// Object of mapped resources with additional information such if they're
    /// live migratable.
    #[serde(rename = "mapped-resource-info")]
    pub mapped_resource_info: serde_json::Value,

    /// List of mapped resources e.g. pci, usb. Deprecated, use
    /// 'mapped-resource-info' instead.
    #[serde(rename = "mapped-resources")]
    pub mapped_resources: Vec<String>,

    /// List of not allowed nodes with additional information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_allowed_nodes: Option<QemuMigratePreconditionsNotAllowedNodes>,

    /// Determines if the VM is running.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    pub running: bool,
}

#[api(
    properties: {
        cdrom: {
            default: false,
        },
        is_unused: {
            default: false,
        },
        size: {
            type: Integer,
        },
        volid: {
            type: String,
        },
    },
)]
/// Object.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct QemuMigratePreconditionsLocalDisks {
    /// True if the disk is a cdrom.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    pub cdrom: bool,

    /// True if the disk is unused.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    pub is_unused: bool,

    /// The size of the disk in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub size: i64,

    /// The volid of the disk.
    pub volid: String,
}

#[api(
    properties: {
        "blocking-ha-resources": {
            items: {
                type: QemuMigratePreconditionsNotAllowedNodesBlockingHaResources,
            },
            optional: true,
            type: Array,
        },
        unavailable_storages: {
            items: {
                description: "A storage",
                type: String,
            },
            optional: true,
            type: Array,
        },
    },
)]
/// List of not allowed nodes with additional information.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct QemuMigratePreconditionsNotAllowedNodes {
    /// HA resources, which are blocking the VM from being migrated to the node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "blocking-ha-resources")]
    pub blocking_ha_resources:
        Option<Vec<QemuMigratePreconditionsNotAllowedNodesBlockingHaResources>>,

    /// A list of not available storages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_storages: Option<Vec<String>>,
}

#[api(
    properties: {
        cause: {
            type: QemuMigratePreconditionsNotAllowedNodesBlockingHaResourcesCause,
        },
        sid: {
            type: String,
        },
    },
)]
/// A blocking HA resource
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct QemuMigratePreconditionsNotAllowedNodesBlockingHaResources {
    pub cause: QemuMigratePreconditionsNotAllowedNodesBlockingHaResourcesCause,

    /// The blocking HA resource id
    pub sid: String,
}

#[api]
/// The reason why the HA resource is blocking the migration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuMigratePreconditionsNotAllowedNodesBlockingHaResourcesCause {
    #[serde(rename = "resource-affinity")]
    /// resource-affinity.
    ResourceAffinity,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(
    QemuMigratePreconditionsNotAllowedNodesBlockingHaResourcesCause
);
serde_plain::derive_fromstr_from_deserialize!(
    QemuMigratePreconditionsNotAllowedNodesBlockingHaResourcesCause
);

const_regex! {

QEMU_MOVE_DISK_STORAGE_RE = r##"^(?i:[a-z][a-z0-9\-_.]*[a-z0-9])$"##;

}

#[test]
fn test_regex_compilation_26() {
    use regex::Regex;
    let _: &Regex = &QEMU_MOVE_DISK_STORAGE_RE;
}
#[api(
    properties: {
        bwlimit: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        delete: {
            default: false,
            optional: true,
        },
        digest: {
            max_length: 40,
            optional: true,
            type: String,
        },
        disk: {
            type: QemuMoveDiskDisk,
        },
        format: {
            optional: true,
            type: QemuMoveDiskFormat,
        },
        storage: {
            format: &ApiStringFormat::Pattern(&QEMU_MOVE_DISK_STORAGE_RE),
            optional: true,
            type: String,
        },
        "target-digest": {
            max_length: 40,
            optional: true,
            type: String,
        },
        "target-disk": {
            optional: true,
            type: QemuMoveDiskDisk,
        },
        "target-vmid": {
            maximum: 999999999,
            minimum: 100,
            optional: true,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuMoveDisk {
    /// Override I/O bandwidth limit (in KiB/s).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bwlimit: Option<u64>,

    /// Delete the original disk after successful copy. By default the original
    /// disk is kept as unused disk.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete: Option<bool>,

    /// Prevent changes if current configuration file has different SHA1 digest.
    /// This can be used to prevent concurrent modifications.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,

    pub disk: QemuMoveDiskDisk,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<QemuMoveDiskFormat>,

    /// Target storage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<String>,

    /// Prevent changes if the current config file of the target VM has a
    /// different SHA1 digest. This can be used to detect concurrent
    /// modifications.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "target-digest")]
    pub target_digest: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "target-disk")]
    pub target_disk: Option<QemuMoveDiskDisk>,

    /// The (unique) ID of the VM.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "target-vmid")]
    pub target_vmid: Option<u32>,
}

#[api]
/// The disk you want to move.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuMoveDiskDisk {
    #[serde(rename = "ide0")]
    /// ide0.
    Ide0,
    #[serde(rename = "ide1")]
    /// ide1.
    Ide1,
    #[serde(rename = "ide2")]
    /// ide2.
    Ide2,
    #[serde(rename = "ide3")]
    /// ide3.
    Ide3,
    #[serde(rename = "scsi0")]
    /// scsi0.
    Scsi0,
    #[serde(rename = "scsi1")]
    /// scsi1.
    Scsi1,
    #[serde(rename = "scsi2")]
    /// scsi2.
    Scsi2,
    #[serde(rename = "scsi3")]
    /// scsi3.
    Scsi3,
    #[serde(rename = "scsi4")]
    /// scsi4.
    Scsi4,
    #[serde(rename = "scsi5")]
    /// scsi5.
    Scsi5,
    #[serde(rename = "scsi6")]
    /// scsi6.
    Scsi6,
    #[serde(rename = "scsi7")]
    /// scsi7.
    Scsi7,
    #[serde(rename = "scsi8")]
    /// scsi8.
    Scsi8,
    #[serde(rename = "scsi9")]
    /// scsi9.
    Scsi9,
    #[serde(rename = "scsi10")]
    /// scsi10.
    Scsi10,
    #[serde(rename = "scsi11")]
    /// scsi11.
    Scsi11,
    #[serde(rename = "scsi12")]
    /// scsi12.
    Scsi12,
    #[serde(rename = "scsi13")]
    /// scsi13.
    Scsi13,
    #[serde(rename = "scsi14")]
    /// scsi14.
    Scsi14,
    #[serde(rename = "scsi15")]
    /// scsi15.
    Scsi15,
    #[serde(rename = "scsi16")]
    /// scsi16.
    Scsi16,
    #[serde(rename = "scsi17")]
    /// scsi17.
    Scsi17,
    #[serde(rename = "scsi18")]
    /// scsi18.
    Scsi18,
    #[serde(rename = "scsi19")]
    /// scsi19.
    Scsi19,
    #[serde(rename = "scsi20")]
    /// scsi20.
    Scsi20,
    #[serde(rename = "scsi21")]
    /// scsi21.
    Scsi21,
    #[serde(rename = "scsi22")]
    /// scsi22.
    Scsi22,
    #[serde(rename = "scsi23")]
    /// scsi23.
    Scsi23,
    #[serde(rename = "scsi24")]
    /// scsi24.
    Scsi24,
    #[serde(rename = "scsi25")]
    /// scsi25.
    Scsi25,
    #[serde(rename = "scsi26")]
    /// scsi26.
    Scsi26,
    #[serde(rename = "scsi27")]
    /// scsi27.
    Scsi27,
    #[serde(rename = "scsi28")]
    /// scsi28.
    Scsi28,
    #[serde(rename = "scsi29")]
    /// scsi29.
    Scsi29,
    #[serde(rename = "scsi30")]
    /// scsi30.
    Scsi30,
    #[serde(rename = "virtio0")]
    /// virtio0.
    Virtio0,
    #[serde(rename = "virtio1")]
    /// virtio1.
    Virtio1,
    #[serde(rename = "virtio2")]
    /// virtio2.
    Virtio2,
    #[serde(rename = "virtio3")]
    /// virtio3.
    Virtio3,
    #[serde(rename = "virtio4")]
    /// virtio4.
    Virtio4,
    #[serde(rename = "virtio5")]
    /// virtio5.
    Virtio5,
    #[serde(rename = "virtio6")]
    /// virtio6.
    Virtio6,
    #[serde(rename = "virtio7")]
    /// virtio7.
    Virtio7,
    #[serde(rename = "virtio8")]
    /// virtio8.
    Virtio8,
    #[serde(rename = "virtio9")]
    /// virtio9.
    Virtio9,
    #[serde(rename = "virtio10")]
    /// virtio10.
    Virtio10,
    #[serde(rename = "virtio11")]
    /// virtio11.
    Virtio11,
    #[serde(rename = "virtio12")]
    /// virtio12.
    Virtio12,
    #[serde(rename = "virtio13")]
    /// virtio13.
    Virtio13,
    #[serde(rename = "virtio14")]
    /// virtio14.
    Virtio14,
    #[serde(rename = "virtio15")]
    /// virtio15.
    Virtio15,
    #[serde(rename = "sata0")]
    /// sata0.
    Sata0,
    #[serde(rename = "sata1")]
    /// sata1.
    Sata1,
    #[serde(rename = "sata2")]
    /// sata2.
    Sata2,
    #[serde(rename = "sata3")]
    /// sata3.
    Sata3,
    #[serde(rename = "sata4")]
    /// sata4.
    Sata4,
    #[serde(rename = "sata5")]
    /// sata5.
    Sata5,
    #[serde(rename = "efidisk0")]
    /// efidisk0.
    Efidisk0,
    #[serde(rename = "tpmstate0")]
    /// tpmstate0.
    Tpmstate0,
    #[serde(rename = "unused0")]
    /// unused0.
    Unused0,
    #[serde(rename = "unused1")]
    /// unused1.
    Unused1,
    #[serde(rename = "unused2")]
    /// unused2.
    Unused2,
    #[serde(rename = "unused3")]
    /// unused3.
    Unused3,
    #[serde(rename = "unused4")]
    /// unused4.
    Unused4,
    #[serde(rename = "unused5")]
    /// unused5.
    Unused5,
    #[serde(rename = "unused6")]
    /// unused6.
    Unused6,
    #[serde(rename = "unused7")]
    /// unused7.
    Unused7,
    #[serde(rename = "unused8")]
    /// unused8.
    Unused8,
    #[serde(rename = "unused9")]
    /// unused9.
    Unused9,
    #[serde(rename = "unused10")]
    /// unused10.
    Unused10,
    #[serde(rename = "unused11")]
    /// unused11.
    Unused11,
    #[serde(rename = "unused12")]
    /// unused12.
    Unused12,
    #[serde(rename = "unused13")]
    /// unused13.
    Unused13,
    #[serde(rename = "unused14")]
    /// unused14.
    Unused14,
    #[serde(rename = "unused15")]
    /// unused15.
    Unused15,
    #[serde(rename = "unused16")]
    /// unused16.
    Unused16,
    #[serde(rename = "unused17")]
    /// unused17.
    Unused17,
    #[serde(rename = "unused18")]
    /// unused18.
    Unused18,
    #[serde(rename = "unused19")]
    /// unused19.
    Unused19,
    #[serde(rename = "unused20")]
    /// unused20.
    Unused20,
    #[serde(rename = "unused21")]
    /// unused21.
    Unused21,
    #[serde(rename = "unused22")]
    /// unused22.
    Unused22,
    #[serde(rename = "unused23")]
    /// unused23.
    Unused23,
    #[serde(rename = "unused24")]
    /// unused24.
    Unused24,
    #[serde(rename = "unused25")]
    /// unused25.
    Unused25,
    #[serde(rename = "unused26")]
    /// unused26.
    Unused26,
    #[serde(rename = "unused27")]
    /// unused27.
    Unused27,
    #[serde(rename = "unused28")]
    /// unused28.
    Unused28,
    #[serde(rename = "unused29")]
    /// unused29.
    Unused29,
    #[serde(rename = "unused30")]
    /// unused30.
    Unused30,
    #[serde(rename = "unused31")]
    /// unused31.
    Unused31,
    #[serde(rename = "unused32")]
    /// unused32.
    Unused32,
    #[serde(rename = "unused33")]
    /// unused33.
    Unused33,
    #[serde(rename = "unused34")]
    /// unused34.
    Unused34,
    #[serde(rename = "unused35")]
    /// unused35.
    Unused35,
    #[serde(rename = "unused36")]
    /// unused36.
    Unused36,
    #[serde(rename = "unused37")]
    /// unused37.
    Unused37,
    #[serde(rename = "unused38")]
    /// unused38.
    Unused38,
    #[serde(rename = "unused39")]
    /// unused39.
    Unused39,
    #[serde(rename = "unused40")]
    /// unused40.
    Unused40,
    #[serde(rename = "unused41")]
    /// unused41.
    Unused41,
    #[serde(rename = "unused42")]
    /// unused42.
    Unused42,
    #[serde(rename = "unused43")]
    /// unused43.
    Unused43,
    #[serde(rename = "unused44")]
    /// unused44.
    Unused44,
    #[serde(rename = "unused45")]
    /// unused45.
    Unused45,
    #[serde(rename = "unused46")]
    /// unused46.
    Unused46,
    #[serde(rename = "unused47")]
    /// unused47.
    Unused47,
    #[serde(rename = "unused48")]
    /// unused48.
    Unused48,
    #[serde(rename = "unused49")]
    /// unused49.
    Unused49,
    #[serde(rename = "unused50")]
    /// unused50.
    Unused50,
    #[serde(rename = "unused51")]
    /// unused51.
    Unused51,
    #[serde(rename = "unused52")]
    /// unused52.
    Unused52,
    #[serde(rename = "unused53")]
    /// unused53.
    Unused53,
    #[serde(rename = "unused54")]
    /// unused54.
    Unused54,
    #[serde(rename = "unused55")]
    /// unused55.
    Unused55,
    #[serde(rename = "unused56")]
    /// unused56.
    Unused56,
    #[serde(rename = "unused57")]
    /// unused57.
    Unused57,
    #[serde(rename = "unused58")]
    /// unused58.
    Unused58,
    #[serde(rename = "unused59")]
    /// unused59.
    Unused59,
    #[serde(rename = "unused60")]
    /// unused60.
    Unused60,
    #[serde(rename = "unused61")]
    /// unused61.
    Unused61,
    #[serde(rename = "unused62")]
    /// unused62.
    Unused62,
    #[serde(rename = "unused63")]
    /// unused63.
    Unused63,
    #[serde(rename = "unused64")]
    /// unused64.
    Unused64,
    #[serde(rename = "unused65")]
    /// unused65.
    Unused65,
    #[serde(rename = "unused66")]
    /// unused66.
    Unused66,
    #[serde(rename = "unused67")]
    /// unused67.
    Unused67,
    #[serde(rename = "unused68")]
    /// unused68.
    Unused68,
    #[serde(rename = "unused69")]
    /// unused69.
    Unused69,
    #[serde(rename = "unused70")]
    /// unused70.
    Unused70,
    #[serde(rename = "unused71")]
    /// unused71.
    Unused71,
    #[serde(rename = "unused72")]
    /// unused72.
    Unused72,
    #[serde(rename = "unused73")]
    /// unused73.
    Unused73,
    #[serde(rename = "unused74")]
    /// unused74.
    Unused74,
    #[serde(rename = "unused75")]
    /// unused75.
    Unused75,
    #[serde(rename = "unused76")]
    /// unused76.
    Unused76,
    #[serde(rename = "unused77")]
    /// unused77.
    Unused77,
    #[serde(rename = "unused78")]
    /// unused78.
    Unused78,
    #[serde(rename = "unused79")]
    /// unused79.
    Unused79,
    #[serde(rename = "unused80")]
    /// unused80.
    Unused80,
    #[serde(rename = "unused81")]
    /// unused81.
    Unused81,
    #[serde(rename = "unused82")]
    /// unused82.
    Unused82,
    #[serde(rename = "unused83")]
    /// unused83.
    Unused83,
    #[serde(rename = "unused84")]
    /// unused84.
    Unused84,
    #[serde(rename = "unused85")]
    /// unused85.
    Unused85,
    #[serde(rename = "unused86")]
    /// unused86.
    Unused86,
    #[serde(rename = "unused87")]
    /// unused87.
    Unused87,
    #[serde(rename = "unused88")]
    /// unused88.
    Unused88,
    #[serde(rename = "unused89")]
    /// unused89.
    Unused89,
    #[serde(rename = "unused90")]
    /// unused90.
    Unused90,
    #[serde(rename = "unused91")]
    /// unused91.
    Unused91,
    #[serde(rename = "unused92")]
    /// unused92.
    Unused92,
    #[serde(rename = "unused93")]
    /// unused93.
    Unused93,
    #[serde(rename = "unused94")]
    /// unused94.
    Unused94,
    #[serde(rename = "unused95")]
    /// unused95.
    Unused95,
    #[serde(rename = "unused96")]
    /// unused96.
    Unused96,
    #[serde(rename = "unused97")]
    /// unused97.
    Unused97,
    #[serde(rename = "unused98")]
    /// unused98.
    Unused98,
    #[serde(rename = "unused99")]
    /// unused99.
    Unused99,
    #[serde(rename = "unused100")]
    /// unused100.
    Unused100,
    #[serde(rename = "unused101")]
    /// unused101.
    Unused101,
    #[serde(rename = "unused102")]
    /// unused102.
    Unused102,
    #[serde(rename = "unused103")]
    /// unused103.
    Unused103,
    #[serde(rename = "unused104")]
    /// unused104.
    Unused104,
    #[serde(rename = "unused105")]
    /// unused105.
    Unused105,
    #[serde(rename = "unused106")]
    /// unused106.
    Unused106,
    #[serde(rename = "unused107")]
    /// unused107.
    Unused107,
    #[serde(rename = "unused108")]
    /// unused108.
    Unused108,
    #[serde(rename = "unused109")]
    /// unused109.
    Unused109,
    #[serde(rename = "unused110")]
    /// unused110.
    Unused110,
    #[serde(rename = "unused111")]
    /// unused111.
    Unused111,
    #[serde(rename = "unused112")]
    /// unused112.
    Unused112,
    #[serde(rename = "unused113")]
    /// unused113.
    Unused113,
    #[serde(rename = "unused114")]
    /// unused114.
    Unused114,
    #[serde(rename = "unused115")]
    /// unused115.
    Unused115,
    #[serde(rename = "unused116")]
    /// unused116.
    Unused116,
    #[serde(rename = "unused117")]
    /// unused117.
    Unused117,
    #[serde(rename = "unused118")]
    /// unused118.
    Unused118,
    #[serde(rename = "unused119")]
    /// unused119.
    Unused119,
    #[serde(rename = "unused120")]
    /// unused120.
    Unused120,
    #[serde(rename = "unused121")]
    /// unused121.
    Unused121,
    #[serde(rename = "unused122")]
    /// unused122.
    Unused122,
    #[serde(rename = "unused123")]
    /// unused123.
    Unused123,
    #[serde(rename = "unused124")]
    /// unused124.
    Unused124,
    #[serde(rename = "unused125")]
    /// unused125.
    Unused125,
    #[serde(rename = "unused126")]
    /// unused126.
    Unused126,
    #[serde(rename = "unused127")]
    /// unused127.
    Unused127,
    #[serde(rename = "unused128")]
    /// unused128.
    Unused128,
    #[serde(rename = "unused129")]
    /// unused129.
    Unused129,
    #[serde(rename = "unused130")]
    /// unused130.
    Unused130,
    #[serde(rename = "unused131")]
    /// unused131.
    Unused131,
    #[serde(rename = "unused132")]
    /// unused132.
    Unused132,
    #[serde(rename = "unused133")]
    /// unused133.
    Unused133,
    #[serde(rename = "unused134")]
    /// unused134.
    Unused134,
    #[serde(rename = "unused135")]
    /// unused135.
    Unused135,
    #[serde(rename = "unused136")]
    /// unused136.
    Unused136,
    #[serde(rename = "unused137")]
    /// unused137.
    Unused137,
    #[serde(rename = "unused138")]
    /// unused138.
    Unused138,
    #[serde(rename = "unused139")]
    /// unused139.
    Unused139,
    #[serde(rename = "unused140")]
    /// unused140.
    Unused140,
    #[serde(rename = "unused141")]
    /// unused141.
    Unused141,
    #[serde(rename = "unused142")]
    /// unused142.
    Unused142,
    #[serde(rename = "unused143")]
    /// unused143.
    Unused143,
    #[serde(rename = "unused144")]
    /// unused144.
    Unused144,
    #[serde(rename = "unused145")]
    /// unused145.
    Unused145,
    #[serde(rename = "unused146")]
    /// unused146.
    Unused146,
    #[serde(rename = "unused147")]
    /// unused147.
    Unused147,
    #[serde(rename = "unused148")]
    /// unused148.
    Unused148,
    #[serde(rename = "unused149")]
    /// unused149.
    Unused149,
    #[serde(rename = "unused150")]
    /// unused150.
    Unused150,
    #[serde(rename = "unused151")]
    /// unused151.
    Unused151,
    #[serde(rename = "unused152")]
    /// unused152.
    Unused152,
    #[serde(rename = "unused153")]
    /// unused153.
    Unused153,
    #[serde(rename = "unused154")]
    /// unused154.
    Unused154,
    #[serde(rename = "unused155")]
    /// unused155.
    Unused155,
    #[serde(rename = "unused156")]
    /// unused156.
    Unused156,
    #[serde(rename = "unused157")]
    /// unused157.
    Unused157,
    #[serde(rename = "unused158")]
    /// unused158.
    Unused158,
    #[serde(rename = "unused159")]
    /// unused159.
    Unused159,
    #[serde(rename = "unused160")]
    /// unused160.
    Unused160,
    #[serde(rename = "unused161")]
    /// unused161.
    Unused161,
    #[serde(rename = "unused162")]
    /// unused162.
    Unused162,
    #[serde(rename = "unused163")]
    /// unused163.
    Unused163,
    #[serde(rename = "unused164")]
    /// unused164.
    Unused164,
    #[serde(rename = "unused165")]
    /// unused165.
    Unused165,
    #[serde(rename = "unused166")]
    /// unused166.
    Unused166,
    #[serde(rename = "unused167")]
    /// unused167.
    Unused167,
    #[serde(rename = "unused168")]
    /// unused168.
    Unused168,
    #[serde(rename = "unused169")]
    /// unused169.
    Unused169,
    #[serde(rename = "unused170")]
    /// unused170.
    Unused170,
    #[serde(rename = "unused171")]
    /// unused171.
    Unused171,
    #[serde(rename = "unused172")]
    /// unused172.
    Unused172,
    #[serde(rename = "unused173")]
    /// unused173.
    Unused173,
    #[serde(rename = "unused174")]
    /// unused174.
    Unused174,
    #[serde(rename = "unused175")]
    /// unused175.
    Unused175,
    #[serde(rename = "unused176")]
    /// unused176.
    Unused176,
    #[serde(rename = "unused177")]
    /// unused177.
    Unused177,
    #[serde(rename = "unused178")]
    /// unused178.
    Unused178,
    #[serde(rename = "unused179")]
    /// unused179.
    Unused179,
    #[serde(rename = "unused180")]
    /// unused180.
    Unused180,
    #[serde(rename = "unused181")]
    /// unused181.
    Unused181,
    #[serde(rename = "unused182")]
    /// unused182.
    Unused182,
    #[serde(rename = "unused183")]
    /// unused183.
    Unused183,
    #[serde(rename = "unused184")]
    /// unused184.
    Unused184,
    #[serde(rename = "unused185")]
    /// unused185.
    Unused185,
    #[serde(rename = "unused186")]
    /// unused186.
    Unused186,
    #[serde(rename = "unused187")]
    /// unused187.
    Unused187,
    #[serde(rename = "unused188")]
    /// unused188.
    Unused188,
    #[serde(rename = "unused189")]
    /// unused189.
    Unused189,
    #[serde(rename = "unused190")]
    /// unused190.
    Unused190,
    #[serde(rename = "unused191")]
    /// unused191.
    Unused191,
    #[serde(rename = "unused192")]
    /// unused192.
    Unused192,
    #[serde(rename = "unused193")]
    /// unused193.
    Unused193,
    #[serde(rename = "unused194")]
    /// unused194.
    Unused194,
    #[serde(rename = "unused195")]
    /// unused195.
    Unused195,
    #[serde(rename = "unused196")]
    /// unused196.
    Unused196,
    #[serde(rename = "unused197")]
    /// unused197.
    Unused197,
    #[serde(rename = "unused198")]
    /// unused198.
    Unused198,
    #[serde(rename = "unused199")]
    /// unused199.
    Unused199,
    #[serde(rename = "unused200")]
    /// unused200.
    Unused200,
    #[serde(rename = "unused201")]
    /// unused201.
    Unused201,
    #[serde(rename = "unused202")]
    /// unused202.
    Unused202,
    #[serde(rename = "unused203")]
    /// unused203.
    Unused203,
    #[serde(rename = "unused204")]
    /// unused204.
    Unused204,
    #[serde(rename = "unused205")]
    /// unused205.
    Unused205,
    #[serde(rename = "unused206")]
    /// unused206.
    Unused206,
    #[serde(rename = "unused207")]
    /// unused207.
    Unused207,
    #[serde(rename = "unused208")]
    /// unused208.
    Unused208,
    #[serde(rename = "unused209")]
    /// unused209.
    Unused209,
    #[serde(rename = "unused210")]
    /// unused210.
    Unused210,
    #[serde(rename = "unused211")]
    /// unused211.
    Unused211,
    #[serde(rename = "unused212")]
    /// unused212.
    Unused212,
    #[serde(rename = "unused213")]
    /// unused213.
    Unused213,
    #[serde(rename = "unused214")]
    /// unused214.
    Unused214,
    #[serde(rename = "unused215")]
    /// unused215.
    Unused215,
    #[serde(rename = "unused216")]
    /// unused216.
    Unused216,
    #[serde(rename = "unused217")]
    /// unused217.
    Unused217,
    #[serde(rename = "unused218")]
    /// unused218.
    Unused218,
    #[serde(rename = "unused219")]
    /// unused219.
    Unused219,
    #[serde(rename = "unused220")]
    /// unused220.
    Unused220,
    #[serde(rename = "unused221")]
    /// unused221.
    Unused221,
    #[serde(rename = "unused222")]
    /// unused222.
    Unused222,
    #[serde(rename = "unused223")]
    /// unused223.
    Unused223,
    #[serde(rename = "unused224")]
    /// unused224.
    Unused224,
    #[serde(rename = "unused225")]
    /// unused225.
    Unused225,
    #[serde(rename = "unused226")]
    /// unused226.
    Unused226,
    #[serde(rename = "unused227")]
    /// unused227.
    Unused227,
    #[serde(rename = "unused228")]
    /// unused228.
    Unused228,
    #[serde(rename = "unused229")]
    /// unused229.
    Unused229,
    #[serde(rename = "unused230")]
    /// unused230.
    Unused230,
    #[serde(rename = "unused231")]
    /// unused231.
    Unused231,
    #[serde(rename = "unused232")]
    /// unused232.
    Unused232,
    #[serde(rename = "unused233")]
    /// unused233.
    Unused233,
    #[serde(rename = "unused234")]
    /// unused234.
    Unused234,
    #[serde(rename = "unused235")]
    /// unused235.
    Unused235,
    #[serde(rename = "unused236")]
    /// unused236.
    Unused236,
    #[serde(rename = "unused237")]
    /// unused237.
    Unused237,
    #[serde(rename = "unused238")]
    /// unused238.
    Unused238,
    #[serde(rename = "unused239")]
    /// unused239.
    Unused239,
    #[serde(rename = "unused240")]
    /// unused240.
    Unused240,
    #[serde(rename = "unused241")]
    /// unused241.
    Unused241,
    #[serde(rename = "unused242")]
    /// unused242.
    Unused242,
    #[serde(rename = "unused243")]
    /// unused243.
    Unused243,
    #[serde(rename = "unused244")]
    /// unused244.
    Unused244,
    #[serde(rename = "unused245")]
    /// unused245.
    Unused245,
    #[serde(rename = "unused246")]
    /// unused246.
    Unused246,
    #[serde(rename = "unused247")]
    /// unused247.
    Unused247,
    #[serde(rename = "unused248")]
    /// unused248.
    Unused248,
    #[serde(rename = "unused249")]
    /// unused249.
    Unused249,
    #[serde(rename = "unused250")]
    /// unused250.
    Unused250,
    #[serde(rename = "unused251")]
    /// unused251.
    Unused251,
    #[serde(rename = "unused252")]
    /// unused252.
    Unused252,
    #[serde(rename = "unused253")]
    /// unused253.
    Unused253,
    #[serde(rename = "unused254")]
    /// unused254.
    Unused254,
    #[serde(rename = "unused255")]
    /// unused255.
    Unused255,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuMoveDiskDisk);
serde_plain::derive_fromstr_from_deserialize!(QemuMoveDiskDisk);

#[api]
/// Target Format.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuMoveDiskFormat {
    #[serde(rename = "raw")]
    /// raw.
    Raw,
    #[serde(rename = "qcow2")]
    /// qcow2.
    Qcow2,
    #[serde(rename = "vmdk")]
    /// vmdk.
    Vmdk,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuMoveDiskFormat);
serde_plain::derive_fromstr_from_deserialize!(QemuMoveDiskFormat);

#[api(
    properties: {
        digest: {
            max_length: 40,
            optional: true,
            type: String,
        },
        disk: {
            type: QemuResizeDisk,
        },
        size: {
            type: String,
        },
        skiplock: {
            default: false,
            optional: true,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuResize {
    /// Prevent changes if current configuration file has different SHA1 digest.
    /// This can be used to prevent concurrent modifications.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,

    pub disk: QemuResizeDisk,

    /// The new size. With the `+` sign the value is added to the actual size of
    /// the volume and without it, the value is taken as an absolute one.
    /// Shrinking disk size is not supported.
    pub size: String,

    /// Ignore locks - only root is allowed to use this option.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skiplock: Option<bool>,
}

#[api]
/// The disk you want to resize.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum QemuResizeDisk {
    #[serde(rename = "ide0")]
    /// ide0.
    Ide0,
    #[serde(rename = "ide1")]
    /// ide1.
    Ide1,
    #[serde(rename = "ide2")]
    /// ide2.
    Ide2,
    #[serde(rename = "ide3")]
    /// ide3.
    Ide3,
    #[serde(rename = "scsi0")]
    /// scsi0.
    Scsi0,
    #[serde(rename = "scsi1")]
    /// scsi1.
    Scsi1,
    #[serde(rename = "scsi2")]
    /// scsi2.
    Scsi2,
    #[serde(rename = "scsi3")]
    /// scsi3.
    Scsi3,
    #[serde(rename = "scsi4")]
    /// scsi4.
    Scsi4,
    #[serde(rename = "scsi5")]
    /// scsi5.
    Scsi5,
    #[serde(rename = "scsi6")]
    /// scsi6.
    Scsi6,
    #[serde(rename = "scsi7")]
    /// scsi7.
    Scsi7,
    #[serde(rename = "scsi8")]
    /// scsi8.
    Scsi8,
    #[serde(rename = "scsi9")]
    /// scsi9.
    Scsi9,
    #[serde(rename = "scsi10")]
    /// scsi10.
    Scsi10,
    #[serde(rename = "scsi11")]
    /// scsi11.
    Scsi11,
    #[serde(rename = "scsi12")]
    /// scsi12.
    Scsi12,
    #[serde(rename = "scsi13")]
    /// scsi13.
    Scsi13,
    #[serde(rename = "scsi14")]
    /// scsi14.
    Scsi14,
    #[serde(rename = "scsi15")]
    /// scsi15.
    Scsi15,
    #[serde(rename = "scsi16")]
    /// scsi16.
    Scsi16,
    #[serde(rename = "scsi17")]
    /// scsi17.
    Scsi17,
    #[serde(rename = "scsi18")]
    /// scsi18.
    Scsi18,
    #[serde(rename = "scsi19")]
    /// scsi19.
    Scsi19,
    #[serde(rename = "scsi20")]
    /// scsi20.
    Scsi20,
    #[serde(rename = "scsi21")]
    /// scsi21.
    Scsi21,
    #[serde(rename = "scsi22")]
    /// scsi22.
    Scsi22,
    #[serde(rename = "scsi23")]
    /// scsi23.
    Scsi23,
    #[serde(rename = "scsi24")]
    /// scsi24.
    Scsi24,
    #[serde(rename = "scsi25")]
    /// scsi25.
    Scsi25,
    #[serde(rename = "scsi26")]
    /// scsi26.
    Scsi26,
    #[serde(rename = "scsi27")]
    /// scsi27.
    Scsi27,
    #[serde(rename = "scsi28")]
    /// scsi28.
    Scsi28,
    #[serde(rename = "scsi29")]
    /// scsi29.
    Scsi29,
    #[serde(rename = "scsi30")]
    /// scsi30.
    Scsi30,
    #[serde(rename = "virtio0")]
    /// virtio0.
    Virtio0,
    #[serde(rename = "virtio1")]
    /// virtio1.
    Virtio1,
    #[serde(rename = "virtio2")]
    /// virtio2.
    Virtio2,
    #[serde(rename = "virtio3")]
    /// virtio3.
    Virtio3,
    #[serde(rename = "virtio4")]
    /// virtio4.
    Virtio4,
    #[serde(rename = "virtio5")]
    /// virtio5.
    Virtio5,
    #[serde(rename = "virtio6")]
    /// virtio6.
    Virtio6,
    #[serde(rename = "virtio7")]
    /// virtio7.
    Virtio7,
    #[serde(rename = "virtio8")]
    /// virtio8.
    Virtio8,
    #[serde(rename = "virtio9")]
    /// virtio9.
    Virtio9,
    #[serde(rename = "virtio10")]
    /// virtio10.
    Virtio10,
    #[serde(rename = "virtio11")]
    /// virtio11.
    Virtio11,
    #[serde(rename = "virtio12")]
    /// virtio12.
    Virtio12,
    #[serde(rename = "virtio13")]
    /// virtio13.
    Virtio13,
    #[serde(rename = "virtio14")]
    /// virtio14.
    Virtio14,
    #[serde(rename = "virtio15")]
    /// virtio15.
    Virtio15,
    #[serde(rename = "sata0")]
    /// sata0.
    Sata0,
    #[serde(rename = "sata1")]
    /// sata1.
    Sata1,
    #[serde(rename = "sata2")]
    /// sata2.
    Sata2,
    #[serde(rename = "sata3")]
    /// sata3.
    Sata3,
    #[serde(rename = "sata4")]
    /// sata4.
    Sata4,
    #[serde(rename = "sata5")]
    /// sata5.
    Sata5,
    #[serde(rename = "efidisk0")]
    /// efidisk0.
    Efidisk0,
    #[serde(rename = "tpmstate0")]
    /// tpmstate0.
    Tpmstate0,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(QemuResizeDisk);
serde_plain::derive_fromstr_from_deserialize!(QemuResizeDisk);

#[api(
    properties: {
        agent: {
            default: false,
            optional: true,
        },
        clipboard: {
            optional: true,
            type: QemuConfigVgaClipboard,
        },
        diskread: {
            optional: true,
            type: Integer,
        },
        diskwrite: {
            optional: true,
            type: Integer,
        },
        ha: {
            description: "HA manager service status.",
            properties: {},
            type: Object,
        },
        lock: {
            optional: true,
            type: String,
        },
        maxdisk: {
            optional: true,
            type: Integer,
        },
        maxmem: {
            optional: true,
            type: Integer,
        },
        mem: {
            optional: true,
            type: Integer,
        },
        memhost: {
            optional: true,
            type: Integer,
        },
        name: {
            optional: true,
            type: String,
        },
        netin: {
            optional: true,
            type: Integer,
        },
        netout: {
            optional: true,
            type: Integer,
        },
        pid: {
            optional: true,
            type: Integer,
        },
        qmpstatus: {
            optional: true,
            type: String,
        },
        "running-machine": {
            optional: true,
            type: String,
        },
        "running-qemu": {
            optional: true,
            type: String,
        },
        serial: {
            default: false,
            optional: true,
        },
        spice: {
            default: false,
            optional: true,
        },
        status: {
            type: IsRunning,
        },
        tags: {
            optional: true,
            type: String,
        },
        template: {
            default: false,
            optional: true,
        },
        uptime: {
            optional: true,
            type: Integer,
        },
        vmid: {
            maximum: 999999999,
            minimum: 100,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QemuStatus {
    /// QEMU Guest Agent is enabled in config.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clipboard: Option<QemuConfigVgaClipboard>,

    /// Current CPU usage.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<f64>,

    /// Maximum usable CPUs.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpus: Option<f64>,

    /// The amount of bytes the guest read from it's block devices since the
    /// guest was started. (Note: This info is not available for all storage
    /// types.)
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diskread: Option<i64>,

    /// The amount of bytes the guest wrote from it's block devices since the
    /// guest was started. (Note: This info is not available for all storage
    /// types.)
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diskwrite: Option<i64>,

    /// HA manager service status.
    pub ha: serde_json::Value,

    /// The current config lock, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<String>,

    /// Root disk size in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxdisk: Option<i64>,

    /// Maximum memory in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxmem: Option<i64>,

    /// Currently used memory in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mem: Option<i64>,

    /// Current memory usage on the host.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memhost: Option<i64>,

    /// VM (host)name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// The amount of traffic in bytes that was sent to the guest over the
    /// network since it was started.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netin: Option<i64>,

    /// The amount of traffic in bytes that was sent from the guest over the
    /// network since it was started.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netout: Option<i64>,

    /// PID of the QEMU process, if the VM is running.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<i64>,

    /// CPU Full pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressurecpufull: Option<f64>,

    /// CPU Some pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressurecpusome: Option<f64>,

    /// IO Full pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressureiofull: Option<f64>,

    /// IO Some pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressureiosome: Option<f64>,

    /// Memory Full pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressurememoryfull: Option<f64>,

    /// Memory Some pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressurememorysome: Option<f64>,

    /// VM run state from the 'query-status' QMP monitor command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qmpstatus: Option<String>,

    /// The currently running machine type (if running).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "running-machine")]
    pub running_machine: Option<String>,

    /// The QEMU version the VM is currently using (if running).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "running-qemu")]
    pub running_qemu: Option<String>,

    /// Guest has serial device configured.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<bool>,

    /// QEMU VGA configuration supports spice.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spice: Option<bool>,

    pub status: IsRunning,

    /// The current configured tags, if any
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,

    /// Determines if the guest is a template.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<bool>,

    /// Uptime in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime: Option<i64>,

    /// The (unique) ID of the VM.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    pub vmid: u32,
}

#[api(
    properties: {
        force: {
            default: false,
            optional: true,
        },
        "lock-token": {
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ReleaseSdnLock {
    /// if true, allow releasing lock without providing the token
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,

    /// the token for unlocking the global SDN configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "lock-token")]
    pub lock_token: Option<String>,
}

#[api(
    properties: {
        "lock-token": {
            optional: true,
            type: String,
        },
        "release-lock": {
            default: true,
            optional: true,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ReloadSdn {
    /// the token for unlocking the global SDN configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "lock-token")]
    pub lock_token: Option<String>,

    /// When lock-token has been provided and configuration successfully
    /// commited, release the lock automatically afterwards
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "release-lock")]
    pub release_lock: Option<bool>,
}

const_regex! {

REMOTE_MIGRATE_LXC_TARGET_BRIDGE_RE = r##"^[-_.\w\d]+:[-_.\w\d]+|[-_.\w\d]+|1$"##;
REMOTE_MIGRATE_LXC_TARGET_STORAGE_RE = r##"^(?i:[a-z][a-z0-9\-_.]*[a-z0-9]):(?i:[a-z][a-z0-9\-_.]*[a-z0-9])|(?i:[a-z][a-z0-9\-_.]*[a-z0-9])|1$"##;

}

#[test]
fn test_regex_compilation_27() {
    use regex::Regex;
    let _: &Regex = &REMOTE_MIGRATE_LXC_TARGET_BRIDGE_RE;
    let _: &Regex = &REMOTE_MIGRATE_LXC_TARGET_STORAGE_RE;
}
#[api(
    properties: {
        bwlimit: {
            minimum: 0.0,
            optional: true,
        },
        delete: {
            default: false,
            optional: true,
        },
        online: {
            default: false,
            optional: true,
        },
        restart: {
            default: false,
            optional: true,
        },
        "target-bridge": {
            items: {
                description: "List item of type bridge-pair.",
                format: &ApiStringFormat::Pattern(&REMOTE_MIGRATE_LXC_TARGET_BRIDGE_RE),
                type: String,
            },
            type: Array,
        },
        "target-endpoint": {
            format: &ApiStringFormat::PropertyString(&ProxmoxRemote::API_SCHEMA),
            type: String,
        },
        "target-storage": {
            items: {
                description: "List item of type storage-pair.",
                format: &ApiStringFormat::Pattern(&REMOTE_MIGRATE_LXC_TARGET_STORAGE_RE),
                type: String,
            },
            type: Array,
        },
        "target-vmid": {
            maximum: 999999999,
            minimum: 100,
            optional: true,
            type: Integer,
        },
        timeout: {
            default: 180,
            optional: true,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct RemoteMigrateLxc {
    /// Override I/O bandwidth limit (in KiB/s).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bwlimit: Option<f64>,

    /// Delete the original CT and related data after successful migration. By
    /// default the original CT is kept on the source cluster in a stopped
    /// state.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete: Option<bool>,

    /// Use online/live migration.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub online: Option<bool>,

    /// Use restart migration
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart: Option<bool>,

    /// Mapping from source to target bridges. Providing only a single bridge ID
    /// maps all source bridges to that bridge. Providing the special value '1'
    /// will map each source bridge to itself.
    #[serde(rename = "target-bridge")]
    pub target_bridge: Vec<String>,

    /// Remote target endpoint
    #[serde(rename = "target-endpoint")]
    pub target_endpoint: String,

    /// Mapping from source to target storages. Providing only a single storage
    /// ID maps all source storages to that storage. Providing the special value
    /// '1' will map each source storage to itself.
    #[serde(rename = "target-storage")]
    pub target_storage: Vec<String>,

    /// The (unique) ID of the VM.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "target-vmid")]
    pub target_vmid: Option<u32>,

    /// Timeout in seconds for shutdown for restart migration
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<i64>,
}

const_regex! {

REMOTE_MIGRATE_QEMU_TARGET_BRIDGE_RE = r##"^[-_.\w\d]+:[-_.\w\d]+|[-_.\w\d]+|1$"##;
REMOTE_MIGRATE_QEMU_TARGET_STORAGE_RE = r##"^(?i:[a-z][a-z0-9\-_.]*[a-z0-9]):(?i:[a-z][a-z0-9\-_.]*[a-z0-9])|(?i:[a-z][a-z0-9\-_.]*[a-z0-9])|1$"##;

}

#[test]
fn test_regex_compilation_28() {
    use regex::Regex;
    let _: &Regex = &REMOTE_MIGRATE_QEMU_TARGET_BRIDGE_RE;
    let _: &Regex = &REMOTE_MIGRATE_QEMU_TARGET_STORAGE_RE;
}
#[api(
    properties: {
        bwlimit: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        delete: {
            default: false,
            optional: true,
        },
        online: {
            default: false,
            optional: true,
        },
        "target-bridge": {
            items: {
                description: "List item of type bridge-pair.",
                format: &ApiStringFormat::Pattern(&REMOTE_MIGRATE_QEMU_TARGET_BRIDGE_RE),
                type: String,
            },
            type: Array,
        },
        "target-endpoint": {
            format: &ApiStringFormat::PropertyString(&ProxmoxRemote::API_SCHEMA),
            type: String,
        },
        "target-storage": {
            items: {
                description: "List item of type storage-pair.",
                format: &ApiStringFormat::Pattern(&REMOTE_MIGRATE_QEMU_TARGET_STORAGE_RE),
                type: String,
            },
            type: Array,
        },
        "target-vmid": {
            maximum: 999999999,
            minimum: 100,
            optional: true,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct RemoteMigrateQemu {
    /// Override I/O bandwidth limit (in KiB/s).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bwlimit: Option<u64>,

    /// Delete the original VM and related data after successful migration. By
    /// default the original VM is kept on the source cluster in a stopped
    /// state.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete: Option<bool>,

    /// Use online/live migration if VM is running. Ignored if VM is stopped.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub online: Option<bool>,

    /// Mapping from source to target bridges. Providing only a single bridge ID
    /// maps all source bridges to that bridge. Providing the special value '1'
    /// will map each source bridge to itself.
    #[serde(rename = "target-bridge")]
    pub target_bridge: Vec<String>,

    /// Remote target endpoint
    #[serde(rename = "target-endpoint")]
    pub target_endpoint: String,

    /// Mapping from source to target storages. Providing only a single storage
    /// ID maps all source storages to that storage. Providing the special value
    /// '1' will map each source storage to itself.
    #[serde(rename = "target-storage")]
    pub target_storage: Vec<String>,

    /// The (unique) ID of the VM.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "target-vmid")]
    pub target_vmid: Option<u32>,
}

#[api(
    properties: {
        "lock-token": {
            optional: true,
            type: String,
        },
        "release-lock": {
            default: true,
            optional: true,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct RollbackSdn {
    /// the token for unlocking the global SDN configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "lock-token")]
    pub lock_token: Option<String>,

    /// When lock-token has been provided and configuration successfully
    /// rollbacked, release the lock automatically afterwards
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "release-lock")]
    pub release_lock: Option<bool>,
}

const STORAGE_INFO_CONTENT: Schema =
    proxmox_schema::ArraySchema::new("list", &StorageContent::API_SCHEMA).schema();

mod storage_info_content {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[doc(hidden)]
    pub trait Ser: Sized {
        fn ser<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>;
        fn de<'de, D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>;
    }

    impl<T: Serialize + for<'a> Deserialize<'a>> Ser for Vec<T> {
        fn ser<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            super::stringlist::serialize(&self[..], serializer, &super::STORAGE_INFO_CONTENT)
        }

        fn de<'de, D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            super::stringlist::deserialize(deserializer, &super::STORAGE_INFO_CONTENT)
        }
    }

    impl<T: Ser> Ser for Option<T> {
        fn ser<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            match self {
                None => serializer.serialize_none(),
                Some(inner) => inner.ser(serializer),
            }
        }

        fn de<'de, D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            use std::fmt;
            use std::marker::PhantomData;

            struct V<T: Ser>(PhantomData<T>);

            impl<'de, T: Ser> serde::de::Visitor<'de> for V<T> {
                type Value = Option<T>;

                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("an optional string")
                }

                fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                    Ok(None)
                }

                fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    T::de(deserializer).map(Some)
                }

                fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                    use serde::de::IntoDeserializer;
                    T::de(value.into_deserializer()).map(Some)
                }
            }

            deserializer.deserialize_option(V::<T>(PhantomData))
        }
    }

    pub fn serialize<T, S>(this: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
        T: Ser,
    {
        this.ser(serializer)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        D: serde::Deserializer<'de>,
        T: Ser,
    {
        T::de(deserializer)
    }
}

const STORAGE_STATUS_CONTENT: Schema =
    proxmox_schema::ArraySchema::new("list", &StorageContent::API_SCHEMA).schema();

mod storage_status_content {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[doc(hidden)]
    pub trait Ser: Sized {
        fn ser<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>;
        fn de<'de, D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>;
    }

    impl<T: Serialize + for<'a> Deserialize<'a>> Ser for Vec<T> {
        fn ser<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            super::stringlist::serialize(&self[..], serializer, &super::STORAGE_STATUS_CONTENT)
        }

        fn de<'de, D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            super::stringlist::deserialize(deserializer, &super::STORAGE_STATUS_CONTENT)
        }
    }

    impl<T: Ser> Ser for Option<T> {
        fn ser<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            match self {
                None => serializer.serialize_none(),
                Some(inner) => inner.ser(serializer),
            }
        }

        fn de<'de, D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            use std::fmt;
            use std::marker::PhantomData;

            struct V<T: Ser>(PhantomData<T>);

            impl<'de, T: Ser> serde::de::Visitor<'de> for V<T> {
                type Value = Option<T>;

                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("an optional string")
                }

                fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                    Ok(None)
                }

                fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    T::de(deserializer).map(Some)
                }

                fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                    use serde::de::IntoDeserializer;
                    T::de(value.into_deserializer()).map(Some)
                }
            }

            deserializer.deserialize_option(V::<T>(PhantomData))
        }
    }

    pub fn serialize<T, S>(this: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
        T: Ser,
    {
        this.ser(serializer)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        D: serde::Deserializer<'de>,
        T: Ser,
    {
        T::de(deserializer)
    }
}

const_regex! {

SDN_CONTROLLER_ISIS_IFACES_RE = r##"^[a-zA-Z][a-zA-Z0-9_]{1,20}([:\.]\d+)?$"##;
SDN_CONTROLLER_ISIS_NET_RE = r##"^[a-fA-F0-9]{2}(\.[a-fA-F0-9]{4}){3,9}\.[a-fA-F0-9]{2}$"##;

}

#[test]
fn test_regex_compilation_29() {
    use regex::Regex;
    let _: &Regex = &SDN_CONTROLLER_ISIS_IFACES_RE;
    let _: &Regex = &SDN_CONTROLLER_ISIS_NET_RE;
}
#[api(
    properties: {
        asn: {
            maximum: 4294967295,
            minimum: 0,
            optional: true,
            type: Integer,
        },
        "bgp-multipath-as-relax": {
            default: false,
            optional: true,
        },
        controller: {
            type: String,
        },
        digest: {
            optional: true,
            type: String,
        },
        ebgp: {
            default: false,
            optional: true,
        },
        "ebgp-multihop": {
            optional: true,
            type: Integer,
        },
        "isis-domain": {
            optional: true,
            type: String,
        },
        "isis-ifaces": {
            format: &ApiStringFormat::Pattern(&SDN_CONTROLLER_ISIS_IFACES_RE),
            optional: true,
            type: String,
        },
        "isis-net": {
            format: &ApiStringFormat::Pattern(&SDN_CONTROLLER_ISIS_NET_RE),
            optional: true,
            type: String,
        },
        loopback: {
            optional: true,
            type: String,
        },
        node: {
            optional: true,
            type: String,
        },
        peers: {
            optional: true,
            type: String,
        },
        pending: {
            optional: true,
            type: SdnControllerPending,
        },
        state: {
            optional: true,
            type: SdnObjectState,
        },
        type: {
            type: ListControllersType,
        },
    },
)]
/// Object.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SdnController {
    /// The local ASN of the controller. BGP & EVPN only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asn: Option<u32>,

    /// Consider different AS paths of equal length for multipath computation.
    /// BGP only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "bgp-multipath-as-relax")]
    pub bgp_multipath_as_relax: Option<bool>,

    /// Name of the controller.
    pub controller: String,

    /// Digest of the controller section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,

    /// Enable eBGP (remote-as external). BGP only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ebgp: Option<bool>,

    /// Set maximum amount of hops for eBGP peers. Needs ebgp set to 1. BGP
    /// only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "ebgp-multihop")]
    pub ebgp_multihop: Option<i64>,

    /// Name of the IS-IS domain. IS-IS only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "isis-domain")]
    pub isis_domain: Option<String>,

    /// Comma-separated list of interfaces where IS-IS should be active. IS-IS
    /// only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "isis-ifaces")]
    pub isis_ifaces: Option<String>,

    /// Network Entity title for this node in the IS-IS network. IS-IS only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "isis-net")]
    pub isis_net: Option<String>,

    /// Name of the loopback/dummy interface that provides the Router-IP. BGP
    /// only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loopback: Option<String>,

    /// Node(s) where this controller is active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,

    /// Comma-separated list of the peers IP addresses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peers: Option<String>,

    /// Changes that have not yet been applied to the running configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<SdnControllerPending>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<SdnObjectState>,

    #[serde(rename = "type")]
    pub ty: ListControllersType,
}

const_regex! {

SDN_CONTROLLER_PENDING_ISIS_IFACES_RE = r##"^[a-zA-Z][a-zA-Z0-9_]{1,20}([:\.]\d+)?$"##;
SDN_CONTROLLER_PENDING_ISIS_NET_RE = r##"^[a-fA-F0-9]{2}(\.[a-fA-F0-9]{4}){3,9}\.[a-fA-F0-9]{2}$"##;

}

#[test]
fn test_regex_compilation_30() {
    use regex::Regex;
    let _: &Regex = &SDN_CONTROLLER_PENDING_ISIS_IFACES_RE;
    let _: &Regex = &SDN_CONTROLLER_PENDING_ISIS_NET_RE;
}
#[api(
    properties: {
        asn: {
            maximum: 4294967295,
            minimum: 0,
            optional: true,
            type: Integer,
        },
        "bgp-multipath-as-relax": {
            default: false,
            optional: true,
        },
        ebgp: {
            default: false,
            optional: true,
        },
        "ebgp-multihop": {
            optional: true,
            type: Integer,
        },
        "isis-domain": {
            optional: true,
            type: String,
        },
        "isis-ifaces": {
            format: &ApiStringFormat::Pattern(&SDN_CONTROLLER_PENDING_ISIS_IFACES_RE),
            optional: true,
            type: String,
        },
        "isis-net": {
            format: &ApiStringFormat::Pattern(&SDN_CONTROLLER_PENDING_ISIS_NET_RE),
            optional: true,
            type: String,
        },
        loopback: {
            optional: true,
            type: String,
        },
        node: {
            optional: true,
            type: String,
        },
        peers: {
            optional: true,
            type: String,
        },
    },
)]
/// Changes that have not yet been applied to the running configuration.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SdnControllerPending {
    /// The local ASN of the controller. BGP & EVPN only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asn: Option<u32>,

    /// Consider different AS paths of equal length for multipath computation.
    /// BGP only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "bgp-multipath-as-relax")]
    pub bgp_multipath_as_relax: Option<bool>,

    /// Enable eBGP (remote-as external). BGP only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ebgp: Option<bool>,

    /// Set maximum amount of hops for eBGP peers. Needs ebgp set to 1. BGP
    /// only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "ebgp-multihop")]
    pub ebgp_multihop: Option<i64>,

    /// Name of the IS-IS domain. IS-IS only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "isis-domain")]
    pub isis_domain: Option<String>,

    /// Comma-separated list of interfaces where IS-IS should be active. IS-IS
    /// only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "isis-ifaces")]
    pub isis_ifaces: Option<String>,

    /// Network Entity title for this node in the IS-IS network. IS-IS only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "isis-net")]
    pub isis_net: Option<String>,

    /// Name of the loopback/dummy interface that provides the Router-IP. BGP
    /// only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loopback: Option<String>,

    /// Node(s) where this controller is active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,

    /// Comma-separated list of the peers IP addresses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peers: Option<String>,
}

#[api]
/// The state of an SDN object.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum SdnObjectState {
    #[serde(rename = "new")]
    /// new.
    New,
    #[serde(rename = "deleted")]
    /// deleted.
    Deleted,
    #[serde(rename = "changed")]
    /// changed.
    Changed,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(SdnObjectState);
serde_plain::derive_fromstr_from_deserialize!(SdnObjectState);

#[api(
    properties: {
        alias: {
            max_length: 256,
            optional: true,
            type: String,
        },
        digest: {
            optional: true,
            type: String,
        },
        "isolate-ports": {
            default: false,
            optional: true,
        },
        pending: {
            optional: true,
            type: SdnVnetPending,
        },
        state: {
            optional: true,
            type: SdnObjectState,
        },
        tag: {
            maximum: 16777215,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        type: {
            type: SdnVnetType,
        },
        vlanaware: {
            default: false,
            optional: true,
        },
        vnet: {
            type: String,
        },
        zone: {
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SdnVnet {
    /// Alias name of the VNet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,

    /// Digest of the VNet section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,

    /// If true, sets the isolated property for all interfaces on the bridge of
    /// this VNet.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "isolate-ports")]
    pub isolate_ports: Option<bool>,

    /// Changes that have not yet been applied to the running configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<SdnVnetPending>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<SdnObjectState>,

    /// VLAN Tag (for VLAN or QinQ zones) or VXLAN VNI (for VXLAN or EVPN
    /// zones).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<u32>,

    #[serde(rename = "type")]
    pub ty: SdnVnetType,

    /// Allow VLANs to pass through this VNet.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlanaware: Option<bool>,

    /// Name of the VNet.
    pub vnet: String,

    /// Name of the zone this VNet belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
}

#[api(
    properties: {
        alias: {
            max_length: 256,
            optional: true,
            type: String,
        },
        "isolate-ports": {
            default: false,
            optional: true,
        },
        tag: {
            maximum: 16777215,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        vlanaware: {
            default: false,
            optional: true,
        },
        zone: {
            optional: true,
            type: String,
        },
    },
)]
/// Changes that have not yet been applied to the running configuration.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SdnVnetPending {
    /// Alias name of the VNet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,

    /// If true, sets the isolated property for all interfaces on the bridge of
    /// this VNet.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "isolate-ports")]
    pub isolate_ports: Option<bool>,

    /// VLAN Tag (for VLAN or QinQ zones) or VXLAN VNI (for VXLAN or EVPN
    /// zones).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<u32>,

    /// Allow VLANs to pass through this VNet.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlanaware: Option<bool>,

    /// Name of the zone this VNet belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
}

#[api]
/// Type of the VNet.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum SdnVnetType {
    #[serde(rename = "vnet")]
    /// vnet.
    Vnet,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(SdnVnetType);
serde_plain::derive_fromstr_from_deserialize!(SdnVnetType);

const_regex! {

SDN_ZONE_EXITNODES_RE = r##"^(?i:[a-z0-9](?i:[a-z0-9\-]*[a-z0-9])?)$"##;
SDN_ZONE_EXITNODES_PRIMARY_RE = r##"^(?i:[a-z0-9](?i:[a-z0-9\-]*[a-z0-9])?)$"##;

}

#[test]
fn test_regex_compilation_31() {
    use regex::Regex;
    let _: &Regex = &SDN_ZONE_EXITNODES_RE;
    let _: &Regex = &SDN_ZONE_EXITNODES_PRIMARY_RE;
}
#[api(
    properties: {
        "advertise-subnets": {
            default: false,
            optional: true,
        },
        bridge: {
            optional: true,
            type: String,
        },
        "bridge-disable-mac-learning": {
            default: false,
            optional: true,
        },
        controller: {
            optional: true,
            type: String,
        },
        dhcp: {
            optional: true,
            type: SdnZoneDhcp,
        },
        digest: {
            optional: true,
            type: String,
        },
        "disable-arp-nd-suppression": {
            default: false,
            optional: true,
        },
        dns: {
            optional: true,
            type: String,
        },
        dnszone: {
            optional: true,
            type: String,
        },
        exitnodes: {
            format: &ApiStringFormat::Pattern(&SDN_ZONE_EXITNODES_RE),
            optional: true,
            type: String,
        },
        "exitnodes-local-routing": {
            default: false,
            optional: true,
        },
        "exitnodes-primary": {
            format: &ApiStringFormat::Pattern(&SDN_ZONE_EXITNODES_PRIMARY_RE),
            optional: true,
            type: String,
        },
        ipam: {
            optional: true,
            type: String,
        },
        mac: {
            optional: true,
            type: String,
        },
        mtu: {
            optional: true,
            type: Integer,
        },
        nodes: {
            optional: true,
            type: String,
        },
        peers: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_ip),
            optional: true,
            type: String,
        },
        pending: {
            optional: true,
            type: SdnZonePending,
        },
        reversedns: {
            optional: true,
            type: String,
        },
        "rt-import": {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_sdn_bgp_rt),
            optional: true,
            type: String,
        },
        state: {
            optional: true,
            type: SdnObjectState,
        },
        tag: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        type: {
            type: ListZonesType,
        },
        "vlan-protocol": {
            optional: true,
            type: NetworkInterfaceVlanProtocol,
        },
        "vrf-vxlan": {
            maximum: 16777215,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        "vxlan-port": {
            default: 4789,
            maximum: 65536,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        zone: {
            type: String,
        },
    },
)]
/// Object.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SdnZone {
    /// Advertise IP prefixes (Type-5 routes) instead of MAC/IP pairs (Type-2
    /// routes). EVPN zone only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "advertise-subnets")]
    pub advertise_subnets: Option<bool>,

    /// the bridge for which VLANs should be managed. VLAN & QinQ zone only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge: Option<String>,

    /// Disable auto mac learning. VLAN zone only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "bridge-disable-mac-learning")]
    pub bridge_disable_mac_learning: Option<bool>,

    /// ID of the controller for this zone. EVPN zone only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controller: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dhcp: Option<SdnZoneDhcp>,

    /// Digest of the controller section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,

    /// Suppress IPv4 ARP && IPv6 Neighbour Discovery messages. EVPN zone only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "disable-arp-nd-suppression")]
    pub disable_arp_nd_suppression: Option<bool>,

    /// ID of the DNS server for this zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns: Option<String>,

    /// Domain name for this zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dnszone: Option<String>,

    /// List of PVE Nodes that should act as exit node for this zone. EVPN zone
    /// only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exitnodes: Option<String>,

    /// Create routes on the exit nodes, so they can connect to EVPN guests.
    /// EVPN zone only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "exitnodes-local-routing")]
    pub exitnodes_local_routing: Option<bool>,

    /// Force traffic through this exitnode first. EVPN zone only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "exitnodes-primary")]
    pub exitnodes_primary: Option<String>,

    /// ID of the IPAM for this zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ipam: Option<String>,

    /// MAC address of the anycast router for this zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,

    /// MTU of the zone, will be used for the created VNet bridges.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtu: Option<i64>,

    /// Nodes where this zone should be created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodes: Option<String>,

    /// Comma-separated list of peers, that are part of the VXLAN zone. Usually
    /// the IPs of the nodes. VXLAN zone only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peers: Option<String>,

    /// Changes that have not yet been applied to the running configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<SdnZonePending>,

    /// ID of the reverse DNS server for this zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reversedns: Option<String>,

    /// Route-Targets that should be imported into the VRF of this zone via BGP.
    /// EVPN zone only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "rt-import")]
    pub rt_import: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<SdnObjectState>,

    /// Service-VLAN Tag (outer VLAN). QinQ zone only
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<u64>,

    #[serde(rename = "type")]
    pub ty: ListZonesType,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vlan-protocol")]
    pub vlan_protocol: Option<NetworkInterfaceVlanProtocol>,

    /// VNI for the zone VRF. EVPN zone only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vrf-vxlan")]
    pub vrf_vxlan: Option<u32>,

    /// UDP port that should be used for the VXLAN tunnel (default 4789). VXLAN
    /// zone only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vxlan-port")]
    pub vxlan_port: Option<u32>,

    /// Name of the zone.
    pub zone: String,
}

#[api]
/// Name of DHCP server backend for this zone.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum SdnZoneDhcp {
    #[serde(rename = "dnsmasq")]
    /// dnsmasq.
    Dnsmasq,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(SdnZoneDhcp);
serde_plain::derive_fromstr_from_deserialize!(SdnZoneDhcp);

const_regex! {

SDN_ZONE_PENDING_EXITNODES_RE = r##"^(?i:[a-z0-9](?i:[a-z0-9\-]*[a-z0-9])?)$"##;
SDN_ZONE_PENDING_EXITNODES_PRIMARY_RE = r##"^(?i:[a-z0-9](?i:[a-z0-9\-]*[a-z0-9])?)$"##;

}

#[test]
fn test_regex_compilation_32() {
    use regex::Regex;
    let _: &Regex = &SDN_ZONE_PENDING_EXITNODES_RE;
    let _: &Regex = &SDN_ZONE_PENDING_EXITNODES_PRIMARY_RE;
}
#[api(
    properties: {
        "advertise-subnets": {
            default: false,
            optional: true,
        },
        bridge: {
            optional: true,
            type: String,
        },
        "bridge-disable-mac-learning": {
            default: false,
            optional: true,
        },
        controller: {
            optional: true,
            type: String,
        },
        dhcp: {
            optional: true,
            type: SdnZoneDhcp,
        },
        "disable-arp-nd-suppression": {
            default: false,
            optional: true,
        },
        dns: {
            optional: true,
            type: String,
        },
        dnszone: {
            optional: true,
            type: String,
        },
        exitnodes: {
            format: &ApiStringFormat::Pattern(&SDN_ZONE_PENDING_EXITNODES_RE),
            optional: true,
            type: String,
        },
        "exitnodes-local-routing": {
            default: false,
            optional: true,
        },
        "exitnodes-primary": {
            format: &ApiStringFormat::Pattern(&SDN_ZONE_PENDING_EXITNODES_PRIMARY_RE),
            optional: true,
            type: String,
        },
        ipam: {
            optional: true,
            type: String,
        },
        mac: {
            optional: true,
            type: String,
        },
        mtu: {
            optional: true,
            type: Integer,
        },
        nodes: {
            optional: true,
            type: String,
        },
        peers: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_ip),
            optional: true,
            type: String,
        },
        reversedns: {
            optional: true,
            type: String,
        },
        "rt-import": {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_sdn_bgp_rt),
            optional: true,
            type: String,
        },
        tag: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        "vlan-protocol": {
            optional: true,
            type: NetworkInterfaceVlanProtocol,
        },
        "vrf-vxlan": {
            maximum: 16777215,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        "vxlan-port": {
            default: 4789,
            maximum: 65536,
            minimum: 1,
            optional: true,
            type: Integer,
        },
    },
)]
/// Changes that have not yet been applied to the running configuration.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SdnZonePending {
    /// Advertise IP prefixes (Type-5 routes) instead of MAC/IP pairs (Type-2
    /// routes). EVPN zone only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "advertise-subnets")]
    pub advertise_subnets: Option<bool>,

    /// the bridge for which VLANs should be managed. VLAN & QinQ zone only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge: Option<String>,

    /// Disable auto mac learning. VLAN zone only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "bridge-disable-mac-learning")]
    pub bridge_disable_mac_learning: Option<bool>,

    /// ID of the controller for this zone. EVPN zone only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controller: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dhcp: Option<SdnZoneDhcp>,

    /// Suppress IPv4 ARP && IPv6 Neighbour Discovery messages. EVPN zone only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "disable-arp-nd-suppression")]
    pub disable_arp_nd_suppression: Option<bool>,

    /// ID of the DNS server for this zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns: Option<String>,

    /// Domain name for this zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dnszone: Option<String>,

    /// List of PVE Nodes that should act as exit node for this zone. EVPN zone
    /// only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exitnodes: Option<String>,

    /// Create routes on the exit nodes, so they can connect to EVPN guests.
    /// EVPN zone only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "exitnodes-local-routing")]
    pub exitnodes_local_routing: Option<bool>,

    /// Force traffic through this exitnode first. EVPN zone only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "exitnodes-primary")]
    pub exitnodes_primary: Option<String>,

    /// ID of the IPAM for this zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ipam: Option<String>,

    /// MAC address of the anycast router for this zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,

    /// MTU of the zone, will be used for the created VNet bridges.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtu: Option<i64>,

    /// Nodes where this zone should be created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodes: Option<String>,

    /// Comma-separated list of peers, that are part of the VXLAN zone. Usually
    /// the IPs of the nodes. VXLAN zone only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peers: Option<String>,

    /// ID of the reverse DNS server for this zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reversedns: Option<String>,

    /// Route-Targets that should be imported into the VRF of this zone via BGP.
    /// EVPN zone only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "rt-import")]
    pub rt_import: Option<String>,

    /// Service-VLAN Tag (outer VLAN). QinQ zone only
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vlan-protocol")]
    pub vlan_protocol: Option<NetworkInterfaceVlanProtocol>,

    /// VNI for the zone VRF. EVPN zone only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vrf-vxlan")]
    pub vrf_vxlan: Option<u32>,

    /// UDP port that should be used for the VXLAN tunnel (default 4789). VXLAN
    /// zone only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "vxlan-port")]
    pub vxlan_port: Option<u32>,
}

#[api(
    properties: {
        forceStop: {
            default: false,
            optional: true,
        },
        timeout: {
            default: 60,
            minimum: 0,
            optional: true,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct ShutdownLxc {
    /// Make sure the Container stops.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "forceStop")]
    pub force_stop: Option<bool>,

    /// Wait maximal timeout seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

#[api(
    properties: {
        forceStop: {
            default: false,
            optional: true,
        },
        keepActive: {
            default: false,
            optional: true,
        },
        skiplock: {
            default: false,
            optional: true,
        },
        timeout: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct ShutdownQemu {
    /// Make sure the VM stops.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "forceStop")]
    pub force_stop: Option<bool>,

    /// Do not deactivate storage volumes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "keepActive")]
    pub keep_active: Option<bool>,

    /// Ignore locks - only root is allowed to use this option.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skiplock: Option<bool>,

    /// Wait maximal timeout seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

#[api(
    properties: {
        debug: {
            default: false,
            optional: true,
        },
        skiplock: {
            default: false,
            optional: true,
        },
    },
)]
/// Object.
#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct StartLxc {
    /// If set, enables very verbose debug log-level on start.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<bool>,

    /// Ignore locks - only root is allowed to use this option.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skiplock: Option<bool>,
}

const_regex! {

START_QEMU_MIGRATEDFROM_RE = r##"^(?i:[a-z0-9](?i:[a-z0-9\-]*[a-z0-9])?)$"##;
START_QEMU_TARGETSTORAGE_RE = r##"^(?i:[a-z][a-z0-9\-_.]*[a-z0-9]):(?i:[a-z][a-z0-9\-_.]*[a-z0-9])|(?i:[a-z][a-z0-9\-_.]*[a-z0-9])|1$"##;

}

#[test]
fn test_regex_compilation_33() {
    use regex::Regex;
    let _: &Regex = &START_QEMU_MIGRATEDFROM_RE;
    let _: &Regex = &START_QEMU_TARGETSTORAGE_RE;
}
#[api(
    properties: {
        "force-cpu": {
            optional: true,
            type: String,
        },
        machine: {
            format: &ApiStringFormat::PropertyString(&QemuConfigMachine::API_SCHEMA),
            optional: true,
            type: String,
        },
        migratedfrom: {
            format: &ApiStringFormat::Pattern(&START_QEMU_MIGRATEDFROM_RE),
            optional: true,
            type: String,
        },
        migration_network: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_cidr),
            optional: true,
            type: String,
        },
        migration_type: {
            optional: true,
            type: StartQemuMigrationType,
        },
        "nets-host-mtu": {
            optional: true,
            type: String,
        },
        skiplock: {
            default: false,
            optional: true,
        },
        stateuri: {
            max_length: 128,
            optional: true,
            type: String,
        },
        targetstorage: {
            items: {
                description: "List item of type storage-pair.",
                format: &ApiStringFormat::Pattern(&START_QEMU_TARGETSTORAGE_RE),
                type: String,
            },
            optional: true,
            type: Array,
        },
        timeout: {
            default: 30,
            minimum: 0,
            optional: true,
            type: Integer,
        },
        "with-conntrack-state": {
            default: false,
            optional: true,
        },
    },
)]
/// Object.
#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct StartQemu {
    /// Override QEMU's -cpu argument with the given string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "force-cpu")]
    pub force_cpu: Option<String>,

    /// Specify the QEMU machine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,

    /// The cluster node name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migratedfrom: Option<String>,

    /// CIDR of the (sub) network that is used for migration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_network: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_type: Option<StartQemuMigrationType>,

    /// Used for migration compat. List of VirtIO network devices and their
    /// effective host_mtu setting according to the QEMU object model on the
    /// source side of the migration. A value of 0 means that the host_mtu
    /// parameter is to be avoided for the corresponding device.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "nets-host-mtu")]
    pub nets_host_mtu: Option<String>,

    /// Ignore locks - only root is allowed to use this option.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skiplock: Option<bool>,

    /// Some command save/restore state from this location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stateuri: Option<String>,

    /// Mapping from source to target storages. Providing only a single storage
    /// ID maps all source storages to that storage. Providing the special value
    /// '1' will map each source storage to itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targetstorage: Option<Vec<String>>,

    /// Wait maximal timeout seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,

    /// Whether to migrate conntrack entries for running VMs.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "with-conntrack-state")]
    pub with_conntrack_state: Option<bool>,
}

#[api]
/// Migration traffic is encrypted using an SSH tunnel by default. On secure,
/// completely private networks this can be disabled to increase performance.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum StartQemuMigrationType {
    #[serde(rename = "secure")]
    /// secure.
    Secure,
    #[serde(rename = "insecure")]
    /// insecure.
    Insecure,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(StartQemuMigrationType);
serde_plain::derive_fromstr_from_deserialize!(StartQemuMigrationType);

#[api(
    properties: {
        "overrule-shutdown": {
            default: false,
            optional: true,
        },
        skiplock: {
            default: false,
            optional: true,
        },
    },
)]
/// Object.
#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct StopLxc {
    /// Try to abort active 'vzshutdown' tasks before stopping.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "overrule-shutdown")]
    pub overrule_shutdown: Option<bool>,

    /// Ignore locks - only root is allowed to use this option.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skiplock: Option<bool>,
}

const_regex! {

STOP_QEMU_MIGRATEDFROM_RE = r##"^(?i:[a-z0-9](?i:[a-z0-9\-]*[a-z0-9])?)$"##;

}

#[test]
fn test_regex_compilation_34() {
    use regex::Regex;
    let _: &Regex = &STOP_QEMU_MIGRATEDFROM_RE;
}
#[api(
    properties: {
        keepActive: {
            default: false,
            optional: true,
        },
        migratedfrom: {
            format: &ApiStringFormat::Pattern(&STOP_QEMU_MIGRATEDFROM_RE),
            optional: true,
            type: String,
        },
        "overrule-shutdown": {
            default: false,
            optional: true,
        },
        skiplock: {
            default: false,
            optional: true,
        },
        timeout: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct StopQemu {
    /// Do not deactivate storage volumes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "keepActive")]
    pub keep_active: Option<bool>,

    /// The cluster node name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migratedfrom: Option<String>,

    /// Try to abort active 'qmshutdown' tasks before stopping.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "overrule-shutdown")]
    pub overrule_shutdown: Option<bool>,

    /// Ignore locks - only root is allowed to use this option.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skiplock: Option<bool>,

    /// Wait maximal timeout seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

#[api]
/// Storage content type.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum StorageContent {
    #[serde(rename = "backup")]
    /// backup.
    Backup,
    #[serde(rename = "images")]
    /// images.
    Images,
    #[serde(rename = "import")]
    /// import.
    Import,
    #[serde(rename = "iso")]
    /// iso.
    Iso,
    #[serde(rename = "none")]
    /// none.
    None,
    #[serde(rename = "rootdir")]
    /// rootdir.
    Rootdir,
    #[serde(rename = "snippets")]
    /// snippets.
    Snippets,
    #[serde(rename = "vztmpl")]
    /// vztmpl.
    Vztmpl,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(StorageContent);
serde_plain::derive_fromstr_from_deserialize!(StorageContent);

const_regex! {

STORAGE_INFO_STORAGE_RE = r##"^(?i:[a-z][a-z0-9\-_.]*[a-z0-9])$"##;

}

#[test]
fn test_regex_compilation_35() {
    use regex::Regex;
    let _: &Regex = &STORAGE_INFO_STORAGE_RE;
}
#[api(
    properties: {
        active: {
            default: false,
            optional: true,
        },
        avail: {
            optional: true,
            type: Integer,
        },
        content: {
            format: &ApiStringFormat::PropertyString(&STORAGE_INFO_CONTENT),
            type: String,
        },
        enabled: {
            default: false,
            optional: true,
        },
        formats: {
            optional: true,
            type: StorageInfoFormats,
        },
        select_existing: {
            default: false,
            optional: true,
        },
        shared: {
            default: false,
            optional: true,
        },
        storage: {
            format: &ApiStringFormat::Pattern(&STORAGE_INFO_STORAGE_RE),
            type: String,
        },
        total: {
            optional: true,
            type: Integer,
        },
        type: {
            type: String,
        },
        used: {
            optional: true,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct StorageInfo {
    /// Set when storage is accessible.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,

    /// Available storage space in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avail: Option<i64>,

    /// Allowed storage content types.
    #[serde(with = "storage_info_content")]
    pub content: Vec<StorageContent>,

    /// Set when storage is enabled (not disabled).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    /// Lists the supported and default format. Use 'formats' instead. Only
    /// included if 'format' parameter is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formats: Option<StorageInfoFormats>,

    /// Instead of creating new volumes, one must select one that is already
    /// existing. Only included if 'format' parameter is set.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select_existing: Option<bool>,

    /// Shared flag from storage configuration.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared: Option<bool>,

    /// The storage identifier.
    pub storage: String,

    /// Total storage space in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,

    /// Storage type.
    #[serde(rename = "type")]
    pub ty: String,

    /// Used storage space in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used: Option<i64>,

    /// Used fraction (used/total).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used_fraction: Option<f64>,
}

#[api(
    properties: {
        default: {
            type: StorageInfoFormatsDefault,
        },
        supported: {
            items: {
                type: StorageInfoFormatsDefault,
            },
            type: Array,
        },
    },
)]
/// Lists the supported and default format. Use 'formats' instead. Only included
/// if 'format' parameter is set.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct StorageInfoFormats {
    pub default: StorageInfoFormatsDefault,

    /// The list of supported formats
    pub supported: Vec<StorageInfoFormatsDefault>,
}

#[api]
/// The default format of the storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum StorageInfoFormatsDefault {
    #[serde(rename = "qcow2")]
    /// qcow2.
    Qcow2,
    #[serde(rename = "raw")]
    /// raw.
    Raw,
    #[serde(rename = "subvol")]
    /// subvol.
    Subvol,
    #[serde(rename = "vmdk")]
    /// vmdk.
    Vmdk,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(StorageInfoFormatsDefault);
serde_plain::derive_fromstr_from_deserialize!(StorageInfoFormatsDefault);

#[api(
    properties: {
        active: {
            default: false,
            optional: true,
        },
        avail: {
            optional: true,
            type: Integer,
        },
        content: {
            format: &ApiStringFormat::PropertyString(&STORAGE_STATUS_CONTENT),
            type: String,
        },
        enabled: {
            default: false,
            optional: true,
        },
        shared: {
            default: false,
            optional: true,
        },
        total: {
            optional: true,
            type: Integer,
        },
        type: {
            type: String,
        },
        used: {
            optional: true,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct StorageStatus {
    /// Set when storage is accessible.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,

    /// Available storage space in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avail: Option<i64>,

    /// Allowed storage content types.
    #[serde(with = "storage_status_content")]
    pub content: Vec<StorageContent>,

    /// Set when storage is enabled (not disabled).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    /// Shared flag from storage configuration.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared: Option<bool>,

    /// Total storage space in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,

    /// Storage type.
    #[serde(rename = "type")]
    pub ty: String,

    /// Used storage space in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used: Option<i64>,
}

#[api(
    properties: {
        n: {
            type: Integer,
        },
        t: {
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct TaskLogLine {
    /// Line number
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub n: i64,

    /// Line text
    pub t: String,
}

#[api(
    properties: {
        exitstatus: {
            optional: true,
            type: String,
            description: "The task's exit status.",
        },
        id: {
            type: String,
            description: "The task id.",
        },
        node: {
            type: String,
            description: "The task's node.",
        },
        pid: {
            type: Integer,
            description: "The task process id.",
        },
        pstart: {
            type: Integer,
            description: "The task's proc start time.",
        },
        starttime: {
            type: Integer,
            description: "The task's start time.",
        },
        status: {
            type: IsRunning,
        },
        type: {
            type: String,
            description: "The task type.",
        },
        upid: {
            type: String,
            description: "The task's UPID.",
        },
        user: {
            type: String,
            description: "The task owner's user id.",
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct TaskStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exitstatus: Option<String>,

    pub id: String,

    pub node: String,

    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub pid: i64,

    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub pstart: i64,

    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    pub starttime: i64,

    pub status: IsRunning,

    #[serde(rename = "type")]
    pub ty: String,

    pub upid: String,

    pub user: String,
}

const_regex! {

UPDATE_QEMU_CONFIG_AFFINITY_RE = r##"^(\s*\d+(-\d+)?\s*)(,\s*\d+(-\d+)?\s*)?$"##;
UPDATE_QEMU_CONFIG_BOOTDISK_RE = r##"^(ide|sata|scsi|virtio|efidisk|tpmstate)\d+$"##;
UPDATE_QEMU_CONFIG_DELETE_RE = r##"^(?i:[a-z][a-z0-9_-]+)$"##;
UPDATE_QEMU_CONFIG_REVERT_RE = r##"^(?i:[a-z][a-z0-9_-]+)$"##;
UPDATE_QEMU_CONFIG_SSHKEYS_RE = r##"^[-%a-zA-Z0-9_.!~*'()]*$"##;
UPDATE_QEMU_CONFIG_TAGS_RE = r##"^(?i)[a-z0-9_][a-z0-9_\-+.]*$"##;
UPDATE_QEMU_CONFIG_VMSTATESTORAGE_RE = r##"^(?i:[a-z][a-z0-9\-_.]*[a-z0-9])$"##;

}

#[test]
fn test_regex_compilation_36() {
    use regex::Regex;
    let _: &Regex = &UPDATE_QEMU_CONFIG_AFFINITY_RE;
    let _: &Regex = &UPDATE_QEMU_CONFIG_BOOTDISK_RE;
    let _: &Regex = &UPDATE_QEMU_CONFIG_DELETE_RE;
    let _: &Regex = &UPDATE_QEMU_CONFIG_REVERT_RE;
    let _: &Regex = &UPDATE_QEMU_CONFIG_SSHKEYS_RE;
    let _: &Regex = &UPDATE_QEMU_CONFIG_TAGS_RE;
    let _: &Regex = &UPDATE_QEMU_CONFIG_VMSTATESTORAGE_RE;
}
#[api(
    properties: {
        acpi: {
            default: true,
            optional: true,
        },
        affinity: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_AFFINITY_RE),
            optional: true,
            type: String,
        },
        agent: {
            format: &ApiStringFormat::PropertyString(&QemuConfigAgent::API_SCHEMA),
            optional: true,
            type: String,
        },
        "amd-sev": {
            format: &ApiStringFormat::PropertyString(&PveQemuSevFmt::API_SCHEMA),
            optional: true,
            type: String,
        },
        arch: {
            optional: true,
            type: QemuConfigArch,
        },
        args: {
            optional: true,
            type: String,
        },
        audio0: {
            format: &ApiStringFormat::PropertyString(&QemuConfigAudio0::API_SCHEMA),
            optional: true,
            type: String,
        },
        autostart: {
            default: false,
            optional: true,
        },
        balloon: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        bios: {
            optional: true,
            type: QemuConfigBios,
        },
        boot: {
            format: &ApiStringFormat::PropertyString(&PveQmBoot::API_SCHEMA),
            optional: true,
            type: String,
        },
        bootdisk: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_BOOTDISK_RE),
            optional: true,
            type: String,
        },
        cdrom: {
            format: &ApiStringFormat::PropertyString(&PveQmIde::API_SCHEMA),
            optional: true,
            type: String,
            type_text: "<volume>",
        },
        cicustom: {
            format: &ApiStringFormat::PropertyString(&PveQmCicustom::API_SCHEMA),
            optional: true,
            type: String,
        },
        cipassword: {
            optional: true,
            type: String,
        },
        citype: {
            optional: true,
            type: QemuConfigCitype,
        },
        ciupgrade: {
            default: true,
            optional: true,
        },
        ciuser: {
            optional: true,
            type: String,
        },
        cores: {
            default: 1,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        cpu: {
            format: &ApiStringFormat::PropertyString(&PveVmCpuConf::API_SCHEMA),
            optional: true,
            type: String,
        },
        cpulimit: {
            default: 0.0,
            maximum: 128.0,
            minimum: 0.0,
            optional: true,
        },
        cpuunits: {
            default: 1024,
            maximum: 262144,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        delete: {
            items: {
                description: "List item of type pve-configid.",
                format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_DELETE_RE),
                type: String,
            },
            optional: true,
            type: Array,
        },
        description: {
            max_length: 8192,
            optional: true,
            type: String,
        },
        digest: {
            max_length: 40,
            optional: true,
            type: String,
        },
        efidisk0: {
            format: &ApiStringFormat::PropertyString(&UpdateQemuConfigEfidisk0::API_SCHEMA),
            optional: true,
            type: String,
        },
        force: {
            default: false,
            optional: true,
        },
        freeze: {
            default: false,
            optional: true,
        },
        hookscript: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_volume_id),
            optional: true,
            type: String,
        },
        hostpci: {
            type: QemuConfigHostpciArray,
        },
        hotplug: {
            default: "network,disk,usb",
            optional: true,
            type: String,
        },
        hugepages: {
            optional: true,
            type: QemuConfigHugepages,
        },
        ide: {
            type: UpdateQemuConfigIdeArray,
        },
        ipconfig: {
            type: QemuConfigIpconfigArray,
        },
        ivshmem: {
            format: &ApiStringFormat::PropertyString(&QemuConfigIvshmem::API_SCHEMA),
            optional: true,
            type: String,
        },
        keephugepages: {
            default: false,
            optional: true,
        },
        keyboard: {
            optional: true,
            type: QemuConfigKeyboard,
        },
        kvm: {
            default: true,
            optional: true,
        },
        localtime: {
            default: false,
            optional: true,
        },
        lock: {
            optional: true,
            type: QemuConfigLock,
        },
        machine: {
            format: &ApiStringFormat::PropertyString(&QemuConfigMachine::API_SCHEMA),
            optional: true,
            type: String,
        },
        memory: {
            format: &ApiStringFormat::PropertyString(&QemuConfigMemory::API_SCHEMA),
            optional: true,
            type: String,
        },
        migrate_downtime: {
            default: 0.1,
            minimum: 0.0,
            optional: true,
        },
        migrate_speed: {
            default: 0,
            minimum: 0,
            optional: true,
            type: Integer,
        },
        name: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_dns_name),
            optional: true,
            type: String,
        },
        nameserver: {
            items: {
                description: "List item of type address.",
                format: &ApiStringFormat::VerifyFn(verifiers::verify_address),
                type: String,
            },
            optional: true,
            type: Array,
        },
        net: {
            type: QemuConfigNetArray,
        },
        numa: {
            default: false,
            optional: true,
        },
        numa_array: {
            type: QemuConfigNumaArray,
        },
        onboot: {
            default: false,
            optional: true,
        },
        ostype: {
            optional: true,
            type: QemuConfigOstype,
        },
        parallel: {
            type: QemuConfigParallelArray,
        },
        protection: {
            default: false,
            optional: true,
        },
        reboot: {
            default: true,
            optional: true,
        },
        revert: {
            items: {
                description: "List item of type pve-configid.",
                format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_REVERT_RE),
                type: String,
            },
            optional: true,
            type: Array,
        },
        rng0: {
            format: &ApiStringFormat::PropertyString(&PveQmRng::API_SCHEMA),
            optional: true,
            type: String,
        },
        sata: {
            type: UpdateQemuConfigSataArray,
        },
        scsi: {
            type: UpdateQemuConfigScsiArray,
        },
        scsihw: {
            optional: true,
            type: QemuConfigScsihw,
        },
        searchdomain: {
            optional: true,
            type: String,
        },
        serial: {
            type: QemuConfigSerialArray,
        },
        shares: {
            default: 1000,
            maximum: 50000,
            minimum: 0,
            optional: true,
            type: Integer,
        },
        skiplock: {
            default: false,
            optional: true,
        },
        smbios1: {
            format: &ApiStringFormat::PropertyString(&PveQmSmbios1::API_SCHEMA),
            max_length: 512,
            optional: true,
            type: String,
        },
        smp: {
            default: 1,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        sockets: {
            default: 1,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        spice_enhancements: {
            format: &ApiStringFormat::PropertyString(&QemuConfigSpiceEnhancements::API_SCHEMA),
            optional: true,
            type: String,
        },
        sshkeys: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_SSHKEYS_RE),
            optional: true,
            type: String,
        },
        startdate: {
            default: "now",
            optional: true,
            type: String,
            type_text: "(now | YYYY-MM-DD | YYYY-MM-DDTHH:MM:SS)",
        },
        startup: {
            optional: true,
            type: String,
            type_text: "[[order=]\\d+] [,up=\\d+] [,down=\\d+] ",
        },
        tablet: {
            default: true,
            optional: true,
        },
        tags: {
            items: {
                description: "List item of type pve-tag.",
                format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_TAGS_RE),
                type: String,
            },
            optional: true,
            type: Array,
        },
        tdf: {
            default: false,
            optional: true,
        },
        template: {
            default: false,
            optional: true,
        },
        tpmstate0: {
            format: &ApiStringFormat::PropertyString(&UpdateQemuConfigTpmstate0::API_SCHEMA),
            optional: true,
            type: String,
        },
        unused: {
            type: QemuConfigUnusedArray,
        },
        usb: {
            type: QemuConfigUsbArray,
        },
        vcpus: {
            default: 0,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        vga: {
            format: &ApiStringFormat::PropertyString(&QemuConfigVga::API_SCHEMA),
            optional: true,
            type: String,
        },
        virtio: {
            type: UpdateQemuConfigVirtioArray,
        },
        virtiofs: {
            type: QemuConfigVirtiofsArray,
        },
        vmgenid: {
            default: "1 (autogenerated)",
            optional: true,
            type: String,
        },
        vmstatestorage: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_VMSTATESTORAGE_RE),
            optional: true,
            type: String,
        },
        watchdog: {
            format: &ApiStringFormat::PropertyString(&PveQmWatchdog::API_SCHEMA),
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct UpdateQemuConfig {
    /// Enable/disable ACPI.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acpi: Option<bool>,

    /// List of host cores used to execute guest processes, for example:
    /// 0,5,8-11
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affinity: Option<String>,

    /// Enable/disable communication with the QEMU Guest Agent and its
    /// properties.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,

    /// Secure Encrypted Virtualization (SEV) features by AMD CPUs
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "amd-sev")]
    pub amd_sev: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<QemuConfigArch>,

    /// Arbitrary arguments passed to kvm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,

    /// Configure a audio device, useful in combination with QXL/Spice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio0: Option<String>,

    /// Automatic restart after crash (currently ignored).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autostart: Option<bool>,

    /// Amount of target RAM for the VM in MiB. Using zero disables the ballon
    /// driver.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balloon: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bios: Option<QemuConfigBios>,

    /// Specify guest boot order. Use the 'order=' sub-property as usage with no
    /// key or 'legacy=' is deprecated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boot: Option<String>,

    /// Enable booting from specified disk. Deprecated: Use 'boot:
    /// order=foo;bar' instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootdisk: Option<String>,

    /// This is an alias for option -ide2
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cdrom: Option<String>,

    /// cloud-init: Specify custom files to replace the automatically generated
    /// ones at start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cicustom: Option<String>,

    /// cloud-init: Password to assign the user. Using this is generally not
    /// recommended. Use ssh keys instead. Also note that older cloud-init
    /// versions do not support hashed passwords.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cipassword: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citype: Option<QemuConfigCitype>,

    /// cloud-init: do an automatic package upgrade after the first boot.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ciupgrade: Option<bool>,

    /// cloud-init: User name to change ssh keys and password for instead of the
    /// image's configured default user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ciuser: Option<String>,

    /// The number of cores per socket.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cores: Option<u64>,

    /// Emulated CPU type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<String>,

    /// Limit of CPU usage.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpulimit: Option<f64>,

    /// CPU weight for a VM, will be clamped to [1, 10000] in cgroup v2.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpuunits: Option<u32>,

    /// A list of settings you want to delete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete: Option<Vec<String>>,

    /// Description for the VM. Shown in the web-interface VM's summary. This is
    /// saved as comment inside the configuration file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Prevent changes if current configuration file has different SHA1 digest.
    /// This can be used to prevent concurrent modifications.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,

    /// Configure a disk for storing EFI vars. Use the special syntax
    /// STORAGE_ID:SIZE_IN_GiB to allocate a new volume. Note that SIZE_IN_GiB
    /// is ignored here and that the default EFI vars are copied to the volume
    /// instead. Use STORAGE_ID:0 and the 'import-from' parameter to import from
    /// an existing volume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub efidisk0: Option<String>,

    /// Force physical removal. Without this, we simple remove the disk from the
    /// config file and create an additional configuration entry called
    /// 'unused[n]', which contains the volume ID. Unlink of unused[n] always
    /// cause physical removal.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,

    /// Freeze CPU at startup (use 'c' monitor command to start execution).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze: Option<bool>,

    /// Script that will be executed during various steps in the vms lifetime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hookscript: Option<String>,

    /// Map host PCI devices into guest.
    #[serde(flatten)]
    pub hostpci: QemuConfigHostpciArray,

    /// Selectively enable hotplug features. This is a comma separated list of
    /// hotplug features: 'network', 'disk', 'cpu', 'memory', 'usb' and
    /// 'cloudinit'. Use '0' to disable hotplug completely. Using '1' as value
    /// is an alias for the default `network,disk,usb`. USB hotplugging is
    /// possible for guests with machine version >= 7.1 and ostype l26 or
    /// windows > 7.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotplug: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hugepages: Option<QemuConfigHugepages>,

    /// Use volume as IDE hard disk or CD-ROM (n is 0 to 3). Use the special
    /// syntax STORAGE_ID:SIZE_IN_GiB to allocate a new volume. Use STORAGE_ID:0
    /// and the 'import-from' parameter to import from an existing volume.
    #[serde(flatten)]
    pub ide: UpdateQemuConfigIdeArray,

    /// cloud-init: Specify IP addresses and gateways for the corresponding
    /// interface.
    ///
    /// IP addresses use CIDR notation, gateways are optional but need an IP of
    /// the same type specified.
    ///
    /// The special string 'dhcp' can be used for IP addresses to use DHCP, in
    /// which case no explicit gateway should be provided.
    /// For IPv6 the special string 'auto' can be used to use stateless
    /// autoconfiguration. This requires cloud-init 19.4 or newer.
    ///
    /// If cloud-init is enabled and neither an IPv4 nor an IPv6 address is
    /// specified, it defaults to using dhcp on IPv4.
    #[serde(flatten)]
    pub ipconfig: QemuConfigIpconfigArray,

    /// Inter-VM shared memory. Useful for direct communication between VMs, or
    /// to the host.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ivshmem: Option<String>,

    /// Use together with hugepages. If enabled, hugepages will not not be
    /// deleted after VM shutdown and can be used for subsequent starts.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keephugepages: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyboard: Option<QemuConfigKeyboard>,

    /// Enable/disable KVM hardware virtualization.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kvm: Option<bool>,

    /// Set the real time clock (RTC) to local time. This is enabled by default
    /// if the `ostype` indicates a Microsoft Windows OS.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub localtime: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<QemuConfigLock>,

    /// Specify the QEMU machine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,

    /// Memory properties.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,

    /// Set maximum tolerated downtime (in seconds) for migrations. Should the
    /// migration not be able to converge in the very end, because too much
    /// newly dirtied RAM needs to be transferred, the limit will be increased
    /// automatically step-by-step until migration can converge.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migrate_downtime: Option<f64>,

    /// Set maximum speed (in MB/s) for migrations. Value 0 is no limit.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migrate_speed: Option<u64>,

    /// Set a name for the VM. Only used on the configuration web interface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// cloud-init: Sets DNS server IP address for a container. Create will
    /// automatically use the setting from the host if neither searchdomain nor
    /// nameserver are set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nameserver: Option<Vec<String>>,

    /// Specify network devices.
    #[serde(flatten)]
    pub net: QemuConfigNetArray,

    /// Enable/disable NUMA.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numa: Option<bool>,

    /// NUMA topology.
    #[serde(flatten)]
    pub numa_array: QemuConfigNumaArray,

    /// Specifies whether a VM will be started during system bootup.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onboot: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ostype: Option<QemuConfigOstype>,

    /// Map host parallel devices (n is 0 to 2).
    #[serde(flatten)]
    pub parallel: QemuConfigParallelArray,

    /// Sets the protection flag of the VM. This will disable the remove VM and
    /// remove disk operations.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protection: Option<bool>,

    /// Allow reboot. If set to '0' the VM exit on reboot.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reboot: Option<bool>,

    /// Revert a pending change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revert: Option<Vec<String>>,

    /// Configure a VirtIO-based Random Number Generator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rng0: Option<String>,

    /// Use volume as SATA hard disk or CD-ROM (n is 0 to 5). Use the special
    /// syntax STORAGE_ID:SIZE_IN_GiB to allocate a new volume. Use STORAGE_ID:0
    /// and the 'import-from' parameter to import from an existing volume.
    #[serde(flatten)]
    pub sata: UpdateQemuConfigSataArray,

    /// Use volume as SCSI hard disk or CD-ROM (n is 0 to 30). Use the special
    /// syntax STORAGE_ID:SIZE_IN_GiB to allocate a new volume. Use STORAGE_ID:0
    /// and the 'import-from' parameter to import from an existing volume.
    #[serde(flatten)]
    pub scsi: UpdateQemuConfigScsiArray,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scsihw: Option<QemuConfigScsihw>,

    /// cloud-init: Sets DNS search domains for a container. Create will
    /// automatically use the setting from the host if neither searchdomain nor
    /// nameserver are set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub searchdomain: Option<String>,

    /// Create a serial device inside the VM (n is 0 to 3)
    #[serde(flatten)]
    pub serial: QemuConfigSerialArray,

    /// Amount of memory shares for auto-ballooning. The larger the number is,
    /// the more memory this VM gets. Number is relative to weights of all other
    /// running VMs. Using zero disables auto-ballooning. Auto-ballooning is
    /// done by pvestatd.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u16")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shares: Option<u16>,

    /// Ignore locks - only root is allowed to use this option.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skiplock: Option<bool>,

    /// Specify SMBIOS type 1 fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smbios1: Option<String>,

    /// The number of CPUs. Please use option -sockets instead.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smp: Option<u64>,

    /// The number of CPU sockets.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sockets: Option<u64>,

    /// Configure additional enhancements for SPICE.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spice_enhancements: Option<String>,

    /// cloud-init: Setup public SSH keys (one key per line, OpenSSH format).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sshkeys: Option<String>,

    /// Set the initial date of the real time clock. Valid format for date
    /// are:'now' or '2006-06-17T16:01:21' or '2006-06-17'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startdate: Option<String>,

    /// Startup and shutdown behavior. Order is a non-negative number defining
    /// the general startup order. Shutdown in done with reverse ordering.
    /// Additionally you can set the 'up' or 'down' delay in seconds, which
    /// specifies a delay to wait before the next VM is started or stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startup: Option<String>,

    /// Enable/disable the USB tablet device.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tablet: Option<bool>,

    /// Tags of the VM. This is only meta information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    /// Enable/disable time drift fix.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tdf: Option<bool>,

    /// Enable/disable Template.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<bool>,

    /// Configure a Disk for storing TPM state. The format is fixed to 'raw'.
    /// Use the special syntax STORAGE_ID:SIZE_IN_GiB to allocate a new volume.
    /// Note that SIZE_IN_GiB is ignored here and 4 MiB will be used instead.
    /// Use STORAGE_ID:0 and the 'import-from' parameter to import from an
    /// existing volume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tpmstate0: Option<String>,

    /// Reference to unused volumes. This is used internally, and should not be
    /// modified manually.
    #[serde(flatten)]
    pub unused: QemuConfigUnusedArray,

    /// Configure an USB device (n is 0 to 4, for machine version >= 7.1 and
    /// ostype l26 or windows > 7, n can be up to 14).
    #[serde(flatten)]
    pub usb: QemuConfigUsbArray,

    /// Number of hotplugged vcpus.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcpus: Option<u64>,

    /// Configure the VGA hardware.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vga: Option<String>,

    /// Use volume as VIRTIO hard disk (n is 0 to 15). Use the special syntax
    /// STORAGE_ID:SIZE_IN_GiB to allocate a new volume. Use STORAGE_ID:0 and
    /// the 'import-from' parameter to import from an existing volume.
    #[serde(flatten)]
    pub virtio: UpdateQemuConfigVirtioArray,

    /// Configuration for sharing a directory between host and guest using
    /// Virtio-fs.
    #[serde(flatten)]
    pub virtiofs: QemuConfigVirtiofsArray,

    /// Set VM Generation ID. Use '1' to autogenerate on create or update, pass
    /// '0' to disable explicitly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vmgenid: Option<String>,

    /// Default storage for VM state volumes/files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vmstatestorage: Option<String>,

    /// Create a virtual hardware watchdog device.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watchdog: Option<String>,
}
generate_array_field! {
    UpdateQemuConfigIdeArray [ 4 ] :
    r#"Use volume as IDE hard disk or CD-ROM (n is 0 to 3). Use the special syntax STORAGE_ID:SIZE_IN_GiB to allocate a new volume. Use STORAGE_ID:0 and the 'import-from' parameter to import from an existing volume."#,
    String => {
        description: "Use volume as IDE hard disk or CD-ROM (n is 0 to 3). Use the special syntax STORAGE_ID:SIZE_IN_GiB to allocate a new volume. Use STORAGE_ID:0 and the 'import-from' parameter to import from an existing volume.",
        format: &ApiStringFormat::PropertyString(&UpdateQemuConfigIde::API_SCHEMA),
        type: String,
    }
    ide
}
generate_array_field! {
    UpdateQemuConfigSataArray [ 6 ] :
    r#"Use volume as SATA hard disk or CD-ROM (n is 0 to 5). Use the special syntax STORAGE_ID:SIZE_IN_GiB to allocate a new volume. Use STORAGE_ID:0 and the 'import-from' parameter to import from an existing volume."#,
    String => {
        description: "Use volume as SATA hard disk or CD-ROM (n is 0 to 5). Use the special syntax STORAGE_ID:SIZE_IN_GiB to allocate a new volume. Use STORAGE_ID:0 and the 'import-from' parameter to import from an existing volume.",
        format: &ApiStringFormat::PropertyString(&UpdateQemuConfigSata::API_SCHEMA),
        type: String,
    }
    sata
}
generate_array_field! {
    UpdateQemuConfigScsiArray [ 31 ] :
    r#"Use volume as SCSI hard disk or CD-ROM (n is 0 to 30). Use the special syntax STORAGE_ID:SIZE_IN_GiB to allocate a new volume. Use STORAGE_ID:0 and the 'import-from' parameter to import from an existing volume."#,
    String => {
        description: "Use volume as SCSI hard disk or CD-ROM (n is 0 to 30). Use the special syntax STORAGE_ID:SIZE_IN_GiB to allocate a new volume. Use STORAGE_ID:0 and the 'import-from' parameter to import from an existing volume.",
        format: &ApiStringFormat::PropertyString(&UpdateQemuConfigScsi::API_SCHEMA),
        type: String,
    }
    scsi
}
generate_array_field! {
    UpdateQemuConfigVirtioArray [ 16 ] :
    r#"Use volume as VIRTIO hard disk (n is 0 to 15). Use the special syntax STORAGE_ID:SIZE_IN_GiB to allocate a new volume. Use STORAGE_ID:0 and the 'import-from' parameter to import from an existing volume."#,
    String => {
        description: "Use volume as VIRTIO hard disk (n is 0 to 15). Use the special syntax STORAGE_ID:SIZE_IN_GiB to allocate a new volume. Use STORAGE_ID:0 and the 'import-from' parameter to import from an existing volume.",
        format: &ApiStringFormat::PropertyString(&UpdateQemuConfigVirtio::API_SCHEMA),
        type: String,
    }
    virtio
}

const_regex! {

UPDATE_QEMU_CONFIG_ASYNC_AFFINITY_RE = r##"^(\s*\d+(-\d+)?\s*)(,\s*\d+(-\d+)?\s*)?$"##;
UPDATE_QEMU_CONFIG_ASYNC_BOOTDISK_RE = r##"^(ide|sata|scsi|virtio|efidisk|tpmstate)\d+$"##;
UPDATE_QEMU_CONFIG_ASYNC_DELETE_RE = r##"^(?i:[a-z][a-z0-9_-]+)$"##;
UPDATE_QEMU_CONFIG_ASYNC_IMPORT_WORKING_STORAGE_RE = r##"^(?i:[a-z][a-z0-9\-_.]*[a-z0-9])$"##;
UPDATE_QEMU_CONFIG_ASYNC_REVERT_RE = r##"^(?i:[a-z][a-z0-9_-]+)$"##;
UPDATE_QEMU_CONFIG_ASYNC_SSHKEYS_RE = r##"^[-%a-zA-Z0-9_.!~*'()]*$"##;
UPDATE_QEMU_CONFIG_ASYNC_TAGS_RE = r##"^(?i)[a-z0-9_][a-z0-9_\-+.]*$"##;
UPDATE_QEMU_CONFIG_ASYNC_VMSTATESTORAGE_RE = r##"^(?i:[a-z][a-z0-9\-_.]*[a-z0-9])$"##;

}

#[test]
fn test_regex_compilation_37() {
    use regex::Regex;
    let _: &Regex = &UPDATE_QEMU_CONFIG_ASYNC_AFFINITY_RE;
    let _: &Regex = &UPDATE_QEMU_CONFIG_ASYNC_BOOTDISK_RE;
    let _: &Regex = &UPDATE_QEMU_CONFIG_ASYNC_DELETE_RE;
    let _: &Regex = &UPDATE_QEMU_CONFIG_ASYNC_IMPORT_WORKING_STORAGE_RE;
    let _: &Regex = &UPDATE_QEMU_CONFIG_ASYNC_REVERT_RE;
    let _: &Regex = &UPDATE_QEMU_CONFIG_ASYNC_SSHKEYS_RE;
    let _: &Regex = &UPDATE_QEMU_CONFIG_ASYNC_TAGS_RE;
    let _: &Regex = &UPDATE_QEMU_CONFIG_ASYNC_VMSTATESTORAGE_RE;
}
#[api(
    properties: {
        acpi: {
            default: true,
            optional: true,
        },
        affinity: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_ASYNC_AFFINITY_RE),
            optional: true,
            type: String,
        },
        agent: {
            format: &ApiStringFormat::PropertyString(&QemuConfigAgent::API_SCHEMA),
            optional: true,
            type: String,
        },
        "amd-sev": {
            format: &ApiStringFormat::PropertyString(&PveQemuSevFmt::API_SCHEMA),
            optional: true,
            type: String,
        },
        arch: {
            optional: true,
            type: QemuConfigArch,
        },
        args: {
            optional: true,
            type: String,
        },
        audio0: {
            format: &ApiStringFormat::PropertyString(&QemuConfigAudio0::API_SCHEMA),
            optional: true,
            type: String,
        },
        autostart: {
            default: false,
            optional: true,
        },
        background_delay: {
            maximum: 30,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        balloon: {
            minimum: 0,
            optional: true,
            type: Integer,
        },
        bios: {
            optional: true,
            type: QemuConfigBios,
        },
        boot: {
            format: &ApiStringFormat::PropertyString(&PveQmBoot::API_SCHEMA),
            optional: true,
            type: String,
        },
        bootdisk: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_ASYNC_BOOTDISK_RE),
            optional: true,
            type: String,
        },
        cdrom: {
            format: &ApiStringFormat::PropertyString(&PveQmIde::API_SCHEMA),
            optional: true,
            type: String,
            type_text: "<volume>",
        },
        cicustom: {
            format: &ApiStringFormat::PropertyString(&PveQmCicustom::API_SCHEMA),
            optional: true,
            type: String,
        },
        cipassword: {
            optional: true,
            type: String,
        },
        citype: {
            optional: true,
            type: QemuConfigCitype,
        },
        ciupgrade: {
            default: true,
            optional: true,
        },
        ciuser: {
            optional: true,
            type: String,
        },
        cores: {
            default: 1,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        cpu: {
            format: &ApiStringFormat::PropertyString(&PveVmCpuConf::API_SCHEMA),
            optional: true,
            type: String,
        },
        cpulimit: {
            default: 0.0,
            maximum: 128.0,
            minimum: 0.0,
            optional: true,
        },
        cpuunits: {
            default: 1024,
            maximum: 262144,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        delete: {
            items: {
                description: "List item of type pve-configid.",
                format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_ASYNC_DELETE_RE),
                type: String,
            },
            optional: true,
            type: Array,
        },
        description: {
            max_length: 8192,
            optional: true,
            type: String,
        },
        digest: {
            max_length: 40,
            optional: true,
            type: String,
        },
        efidisk0: {
            format: &ApiStringFormat::PropertyString(&UpdateQemuConfigEfidisk0::API_SCHEMA),
            optional: true,
            type: String,
        },
        force: {
            default: false,
            optional: true,
        },
        freeze: {
            default: false,
            optional: true,
        },
        hookscript: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_volume_id),
            optional: true,
            type: String,
        },
        hostpci: {
            type: QemuConfigHostpciArray,
        },
        hotplug: {
            default: "network,disk,usb",
            optional: true,
            type: String,
        },
        hugepages: {
            optional: true,
            type: QemuConfigHugepages,
        },
        ide: {
            type: UpdateQemuConfigIdeArray,
        },
        "import-working-storage": {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_ASYNC_IMPORT_WORKING_STORAGE_RE),
            optional: true,
            type: String,
        },
        ipconfig: {
            type: QemuConfigIpconfigArray,
        },
        ivshmem: {
            format: &ApiStringFormat::PropertyString(&QemuConfigIvshmem::API_SCHEMA),
            optional: true,
            type: String,
        },
        keephugepages: {
            default: false,
            optional: true,
        },
        keyboard: {
            optional: true,
            type: QemuConfigKeyboard,
        },
        kvm: {
            default: true,
            optional: true,
        },
        localtime: {
            default: false,
            optional: true,
        },
        lock: {
            optional: true,
            type: QemuConfigLock,
        },
        machine: {
            format: &ApiStringFormat::PropertyString(&QemuConfigMachine::API_SCHEMA),
            optional: true,
            type: String,
        },
        memory: {
            format: &ApiStringFormat::PropertyString(&QemuConfigMemory::API_SCHEMA),
            optional: true,
            type: String,
        },
        migrate_downtime: {
            default: 0.1,
            minimum: 0.0,
            optional: true,
        },
        migrate_speed: {
            default: 0,
            minimum: 0,
            optional: true,
            type: Integer,
        },
        name: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_dns_name),
            optional: true,
            type: String,
        },
        nameserver: {
            items: {
                description: "List item of type address.",
                format: &ApiStringFormat::VerifyFn(verifiers::verify_address),
                type: String,
            },
            optional: true,
            type: Array,
        },
        net: {
            type: QemuConfigNetArray,
        },
        numa: {
            default: false,
            optional: true,
        },
        numa_array: {
            type: QemuConfigNumaArray,
        },
        onboot: {
            default: false,
            optional: true,
        },
        ostype: {
            optional: true,
            type: QemuConfigOstype,
        },
        parallel: {
            type: QemuConfigParallelArray,
        },
        protection: {
            default: false,
            optional: true,
        },
        reboot: {
            default: true,
            optional: true,
        },
        revert: {
            items: {
                description: "List item of type pve-configid.",
                format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_ASYNC_REVERT_RE),
                type: String,
            },
            optional: true,
            type: Array,
        },
        rng0: {
            format: &ApiStringFormat::PropertyString(&PveQmRng::API_SCHEMA),
            optional: true,
            type: String,
        },
        sata: {
            type: UpdateQemuConfigSataArray,
        },
        scsi: {
            type: UpdateQemuConfigScsiArray,
        },
        scsihw: {
            optional: true,
            type: QemuConfigScsihw,
        },
        searchdomain: {
            optional: true,
            type: String,
        },
        serial: {
            type: QemuConfigSerialArray,
        },
        shares: {
            default: 1000,
            maximum: 50000,
            minimum: 0,
            optional: true,
            type: Integer,
        },
        skiplock: {
            default: false,
            optional: true,
        },
        smbios1: {
            format: &ApiStringFormat::PropertyString(&PveQmSmbios1::API_SCHEMA),
            max_length: 512,
            optional: true,
            type: String,
        },
        smp: {
            default: 1,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        sockets: {
            default: 1,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        spice_enhancements: {
            format: &ApiStringFormat::PropertyString(&QemuConfigSpiceEnhancements::API_SCHEMA),
            optional: true,
            type: String,
        },
        sshkeys: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_ASYNC_SSHKEYS_RE),
            optional: true,
            type: String,
        },
        startdate: {
            default: "now",
            optional: true,
            type: String,
            type_text: "(now | YYYY-MM-DD | YYYY-MM-DDTHH:MM:SS)",
        },
        startup: {
            optional: true,
            type: String,
            type_text: "[[order=]\\d+] [,up=\\d+] [,down=\\d+] ",
        },
        tablet: {
            default: true,
            optional: true,
        },
        tags: {
            items: {
                description: "List item of type pve-tag.",
                format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_ASYNC_TAGS_RE),
                type: String,
            },
            optional: true,
            type: Array,
        },
        tdf: {
            default: false,
            optional: true,
        },
        template: {
            default: false,
            optional: true,
        },
        tpmstate0: {
            format: &ApiStringFormat::PropertyString(&UpdateQemuConfigTpmstate0::API_SCHEMA),
            optional: true,
            type: String,
        },
        unused: {
            type: QemuConfigUnusedArray,
        },
        usb: {
            type: QemuConfigUsbArray,
        },
        vcpus: {
            default: 0,
            minimum: 1,
            optional: true,
            type: Integer,
        },
        vga: {
            format: &ApiStringFormat::PropertyString(&QemuConfigVga::API_SCHEMA),
            optional: true,
            type: String,
        },
        virtio: {
            type: UpdateQemuConfigVirtioArray,
        },
        virtiofs: {
            type: QemuConfigVirtiofsArray,
        },
        vmgenid: {
            default: "1 (autogenerated)",
            optional: true,
            type: String,
        },
        vmstatestorage: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_ASYNC_VMSTATESTORAGE_RE),
            optional: true,
            type: String,
        },
        watchdog: {
            format: &ApiStringFormat::PropertyString(&PveQmWatchdog::API_SCHEMA),
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct UpdateQemuConfigAsync {
    /// Enable/disable ACPI.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acpi: Option<bool>,

    /// List of host cores used to execute guest processes, for example:
    /// 0,5,8-11
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affinity: Option<String>,

    /// Enable/disable communication with the QEMU Guest Agent and its
    /// properties.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,

    /// Secure Encrypted Virtualization (SEV) features by AMD CPUs
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "amd-sev")]
    pub amd_sev: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<QemuConfigArch>,

    /// Arbitrary arguments passed to kvm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,

    /// Configure a audio device, useful in combination with QXL/Spice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio0: Option<String>,

    /// Automatic restart after crash (currently ignored).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autostart: Option<bool>,

    /// Time to wait for the task to finish. We return 'null' if the task finish
    /// within that time.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u8")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_delay: Option<u8>,

    /// Amount of target RAM for the VM in MiB. Using zero disables the ballon
    /// driver.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balloon: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bios: Option<QemuConfigBios>,

    /// Specify guest boot order. Use the 'order=' sub-property as usage with no
    /// key or 'legacy=' is deprecated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boot: Option<String>,

    /// Enable booting from specified disk. Deprecated: Use 'boot:
    /// order=foo;bar' instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootdisk: Option<String>,

    /// This is an alias for option -ide2
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cdrom: Option<String>,

    /// cloud-init: Specify custom files to replace the automatically generated
    /// ones at start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cicustom: Option<String>,

    /// cloud-init: Password to assign the user. Using this is generally not
    /// recommended. Use ssh keys instead. Also note that older cloud-init
    /// versions do not support hashed passwords.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cipassword: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citype: Option<QemuConfigCitype>,

    /// cloud-init: do an automatic package upgrade after the first boot.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ciupgrade: Option<bool>,

    /// cloud-init: User name to change ssh keys and password for instead of the
    /// image's configured default user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ciuser: Option<String>,

    /// The number of cores per socket.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cores: Option<u64>,

    /// Emulated CPU type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<String>,

    /// Limit of CPU usage.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpulimit: Option<f64>,

    /// CPU weight for a VM, will be clamped to [1, 10000] in cgroup v2.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpuunits: Option<u32>,

    /// A list of settings you want to delete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete: Option<Vec<String>>,

    /// Description for the VM. Shown in the web-interface VM's summary. This is
    /// saved as comment inside the configuration file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Prevent changes if current configuration file has different SHA1 digest.
    /// This can be used to prevent concurrent modifications.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,

    /// Configure a disk for storing EFI vars. Use the special syntax
    /// STORAGE_ID:SIZE_IN_GiB to allocate a new volume. Note that SIZE_IN_GiB
    /// is ignored here and that the default EFI vars are copied to the volume
    /// instead. Use STORAGE_ID:0 and the 'import-from' parameter to import from
    /// an existing volume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub efidisk0: Option<String>,

    /// Force physical removal. Without this, we simple remove the disk from the
    /// config file and create an additional configuration entry called
    /// 'unused[n]', which contains the volume ID. Unlink of unused[n] always
    /// cause physical removal.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,

    /// Freeze CPU at startup (use 'c' monitor command to start execution).
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze: Option<bool>,

    /// Script that will be executed during various steps in the vms lifetime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hookscript: Option<String>,

    /// Map host PCI devices into guest.
    #[serde(flatten)]
    pub hostpci: QemuConfigHostpciArray,

    /// Selectively enable hotplug features. This is a comma separated list of
    /// hotplug features: 'network', 'disk', 'cpu', 'memory', 'usb' and
    /// 'cloudinit'. Use '0' to disable hotplug completely. Using '1' as value
    /// is an alias for the default `network,disk,usb`. USB hotplugging is
    /// possible for guests with machine version >= 7.1 and ostype l26 or
    /// windows > 7.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotplug: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hugepages: Option<QemuConfigHugepages>,

    /// Use volume as IDE hard disk or CD-ROM (n is 0 to 3). Use the special
    /// syntax STORAGE_ID:SIZE_IN_GiB to allocate a new volume. Use STORAGE_ID:0
    /// and the 'import-from' parameter to import from an existing volume.
    #[serde(flatten)]
    pub ide: UpdateQemuConfigIdeArray,

    /// A file-based storage with 'images' content-type enabled, which is used
    /// as an intermediary extraction storage during import. Defaults to the
    /// source storage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "import-working-storage")]
    pub import_working_storage: Option<String>,

    /// cloud-init: Specify IP addresses and gateways for the corresponding
    /// interface.
    ///
    /// IP addresses use CIDR notation, gateways are optional but need an IP of
    /// the same type specified.
    ///
    /// The special string 'dhcp' can be used for IP addresses to use DHCP, in
    /// which case no explicit gateway should be provided.
    /// For IPv6 the special string 'auto' can be used to use stateless
    /// autoconfiguration. This requires cloud-init 19.4 or newer.
    ///
    /// If cloud-init is enabled and neither an IPv4 nor an IPv6 address is
    /// specified, it defaults to using dhcp on IPv4.
    #[serde(flatten)]
    pub ipconfig: QemuConfigIpconfigArray,

    /// Inter-VM shared memory. Useful for direct communication between VMs, or
    /// to the host.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ivshmem: Option<String>,

    /// Use together with hugepages. If enabled, hugepages will not not be
    /// deleted after VM shutdown and can be used for subsequent starts.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keephugepages: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyboard: Option<QemuConfigKeyboard>,

    /// Enable/disable KVM hardware virtualization.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kvm: Option<bool>,

    /// Set the real time clock (RTC) to local time. This is enabled by default
    /// if the `ostype` indicates a Microsoft Windows OS.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub localtime: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<QemuConfigLock>,

    /// Specify the QEMU machine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,

    /// Memory properties.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,

    /// Set maximum tolerated downtime (in seconds) for migrations. Should the
    /// migration not be able to converge in the very end, because too much
    /// newly dirtied RAM needs to be transferred, the limit will be increased
    /// automatically step-by-step until migration can converge.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migrate_downtime: Option<f64>,

    /// Set maximum speed (in MB/s) for migrations. Value 0 is no limit.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migrate_speed: Option<u64>,

    /// Set a name for the VM. Only used on the configuration web interface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// cloud-init: Sets DNS server IP address for a container. Create will
    /// automatically use the setting from the host if neither searchdomain nor
    /// nameserver are set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nameserver: Option<Vec<String>>,

    /// Specify network devices.
    #[serde(flatten)]
    pub net: QemuConfigNetArray,

    /// Enable/disable NUMA.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numa: Option<bool>,

    /// NUMA topology.
    #[serde(flatten)]
    pub numa_array: QemuConfigNumaArray,

    /// Specifies whether a VM will be started during system bootup.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onboot: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ostype: Option<QemuConfigOstype>,

    /// Map host parallel devices (n is 0 to 2).
    #[serde(flatten)]
    pub parallel: QemuConfigParallelArray,

    /// Sets the protection flag of the VM. This will disable the remove VM and
    /// remove disk operations.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protection: Option<bool>,

    /// Allow reboot. If set to '0' the VM exit on reboot.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reboot: Option<bool>,

    /// Revert a pending change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revert: Option<Vec<String>>,

    /// Configure a VirtIO-based Random Number Generator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rng0: Option<String>,

    /// Use volume as SATA hard disk or CD-ROM (n is 0 to 5). Use the special
    /// syntax STORAGE_ID:SIZE_IN_GiB to allocate a new volume. Use STORAGE_ID:0
    /// and the 'import-from' parameter to import from an existing volume.
    #[serde(flatten)]
    pub sata: UpdateQemuConfigSataArray,

    /// Use volume as SCSI hard disk or CD-ROM (n is 0 to 30). Use the special
    /// syntax STORAGE_ID:SIZE_IN_GiB to allocate a new volume. Use STORAGE_ID:0
    /// and the 'import-from' parameter to import from an existing volume.
    #[serde(flatten)]
    pub scsi: UpdateQemuConfigScsiArray,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scsihw: Option<QemuConfigScsihw>,

    /// cloud-init: Sets DNS search domains for a container. Create will
    /// automatically use the setting from the host if neither searchdomain nor
    /// nameserver are set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub searchdomain: Option<String>,

    /// Create a serial device inside the VM (n is 0 to 3)
    #[serde(flatten)]
    pub serial: QemuConfigSerialArray,

    /// Amount of memory shares for auto-ballooning. The larger the number is,
    /// the more memory this VM gets. Number is relative to weights of all other
    /// running VMs. Using zero disables auto-ballooning. Auto-ballooning is
    /// done by pvestatd.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u16")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shares: Option<u16>,

    /// Ignore locks - only root is allowed to use this option.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skiplock: Option<bool>,

    /// Specify SMBIOS type 1 fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smbios1: Option<String>,

    /// The number of CPUs. Please use option -sockets instead.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smp: Option<u64>,

    /// The number of CPU sockets.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sockets: Option<u64>,

    /// Configure additional enhancements for SPICE.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spice_enhancements: Option<String>,

    /// cloud-init: Setup public SSH keys (one key per line, OpenSSH format).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sshkeys: Option<String>,

    /// Set the initial date of the real time clock. Valid format for date
    /// are:'now' or '2006-06-17T16:01:21' or '2006-06-17'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startdate: Option<String>,

    /// Startup and shutdown behavior. Order is a non-negative number defining
    /// the general startup order. Shutdown in done with reverse ordering.
    /// Additionally you can set the 'up' or 'down' delay in seconds, which
    /// specifies a delay to wait before the next VM is started or stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startup: Option<String>,

    /// Enable/disable the USB tablet device.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tablet: Option<bool>,

    /// Tags of the VM. This is only meta information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    /// Enable/disable time drift fix.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tdf: Option<bool>,

    /// Enable/disable Template.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<bool>,

    /// Configure a Disk for storing TPM state. The format is fixed to 'raw'.
    /// Use the special syntax STORAGE_ID:SIZE_IN_GiB to allocate a new volume.
    /// Note that SIZE_IN_GiB is ignored here and 4 MiB will be used instead.
    /// Use STORAGE_ID:0 and the 'import-from' parameter to import from an
    /// existing volume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tpmstate0: Option<String>,

    /// Reference to unused volumes. This is used internally, and should not be
    /// modified manually.
    #[serde(flatten)]
    pub unused: QemuConfigUnusedArray,

    /// Configure an USB device (n is 0 to 4, for machine version >= 7.1 and
    /// ostype l26 or windows > 7, n can be up to 14).
    #[serde(flatten)]
    pub usb: QemuConfigUsbArray,

    /// Number of hotplugged vcpus.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcpus: Option<u64>,

    /// Configure the VGA hardware.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vga: Option<String>,

    /// Use volume as VIRTIO hard disk (n is 0 to 15). Use the special syntax
    /// STORAGE_ID:SIZE_IN_GiB to allocate a new volume. Use STORAGE_ID:0 and
    /// the 'import-from' parameter to import from an existing volume.
    #[serde(flatten)]
    pub virtio: UpdateQemuConfigVirtioArray,

    /// Configuration for sharing a directory between host and guest using
    /// Virtio-fs.
    #[serde(flatten)]
    pub virtiofs: QemuConfigVirtiofsArray,

    /// Set VM Generation ID. Use '1' to autogenerate on create or update, pass
    /// '0' to disable explicitly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vmgenid: Option<String>,

    /// Default storage for VM state volumes/files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vmstatestorage: Option<String>,

    /// Create a virtual hardware watchdog device.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watchdog: Option<String>,
}

const_regex! {

UPDATE_QEMU_CONFIG_EFIDISK0_SIZE_RE = r##"^(\d+(\.\d+)?)([KMGT])?$"##;

}

#[test]
fn test_regex_compilation_38() {
    use regex::Regex;
    let _: &Regex = &UPDATE_QEMU_CONFIG_EFIDISK0_SIZE_RE;
}
#[api(
    default_key: "file",
    properties: {
        efitype: {
            optional: true,
            type: QemuConfigEfidisk0Efitype,
        },
        file: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_volume_id_or_qm_path),
            type: String,
        },
        format: {
            optional: true,
            type: PveQmIdeFormat,
        },
        "import-from": {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_volume_id_or_absolute_path),
            optional: true,
            type: String,
        },
        "pre-enrolled-keys": {
            default: false,
            optional: true,
        },
        size: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_EFIDISK0_SIZE_RE),
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct UpdateQemuConfigEfidisk0 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub efitype: Option<QemuConfigEfidisk0Efitype>,

    /// The drive's backing volume.
    pub file: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<PveQmIdeFormat>,

    /// Create a new disk, importing from this source (volume ID or absolute
    /// path). When an absolute path is specified, it's up to you to ensure that
    /// the source is not actively used by another process during the import!
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "import-from")]
    pub import_from: Option<String>,

    /// Use am EFI vars template with distribution-specific and Microsoft
    /// Standard keys enrolled, if used with 'efitype=4m'. Note that this will
    /// enable Secure Boot by default, though it can still be turned off from
    /// within the VM.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "pre-enrolled-keys")]
    pub pre_enrolled_keys: Option<bool>,

    /// Disk size. This is purely informational and has no effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
}

const_regex! {

UPDATE_QEMU_CONFIG_IDE_MODEL_RE = r##"^[-%a-zA-Z0-9_.!~*'()]*$"##;
UPDATE_QEMU_CONFIG_IDE_SERIAL_RE = r##"^[-%a-zA-Z0-9_.!~*'()]*$"##;
UPDATE_QEMU_CONFIG_IDE_SIZE_RE = r##"^(\d+(\.\d+)?)([KMGT])?$"##;

}

#[test]
fn test_regex_compilation_39() {
    use regex::Regex;
    let _: &Regex = &UPDATE_QEMU_CONFIG_IDE_MODEL_RE;
    let _: &Regex = &UPDATE_QEMU_CONFIG_IDE_SERIAL_RE;
    let _: &Regex = &UPDATE_QEMU_CONFIG_IDE_SIZE_RE;
}
#[api(
    default_key: "file",
    properties: {
        aio: {
            optional: true,
            type: PveQmIdeAio,
        },
        backup: {
            default: false,
            optional: true,
        },
        bps: {
            optional: true,
            type: Integer,
        },
        bps_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        bps_rd: {
            optional: true,
            type: Integer,
        },
        bps_rd_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        bps_wr: {
            optional: true,
            type: Integer,
        },
        bps_wr_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        cache: {
            optional: true,
            type: PveQmIdeCache,
        },
        detect_zeroes: {
            default: false,
            optional: true,
        },
        discard: {
            optional: true,
            type: PveQmIdeDiscard,
        },
        file: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_volume_id_or_qm_path),
            type: String,
        },
        format: {
            optional: true,
            type: PveQmIdeFormat,
        },
        "import-from": {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_volume_id_or_absolute_path),
            optional: true,
            type: String,
        },
        iops: {
            optional: true,
            type: Integer,
        },
        iops_max: {
            optional: true,
            type: Integer,
        },
        iops_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iops_rd: {
            optional: true,
            type: Integer,
        },
        iops_rd_max: {
            optional: true,
            type: Integer,
        },
        iops_rd_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iops_wr: {
            optional: true,
            type: Integer,
        },
        iops_wr_max: {
            optional: true,
            type: Integer,
        },
        iops_wr_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        media: {
            optional: true,
            type: PveQmIdeMedia,
        },
        model: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_IDE_MODEL_RE),
            max_length: 120,
            optional: true,
            type: String,
        },
        replicate: {
            default: true,
            optional: true,
        },
        rerror: {
            optional: true,
            type: PveQmIdeRerror,
        },
        serial: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_IDE_SERIAL_RE),
            max_length: 60,
            optional: true,
            type: String,
        },
        shared: {
            default: false,
            optional: true,
        },
        size: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_IDE_SIZE_RE),
            optional: true,
            type: String,
        },
        snapshot: {
            default: false,
            optional: true,
        },
        ssd: {
            default: false,
            optional: true,
        },
        werror: {
            optional: true,
            type: PveQmIdeWerror,
        },
        wwn: {
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct UpdateQemuConfigIde {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aio: Option<PveQmIdeAio>,

    /// Whether the drive should be included when making backups.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<bool>,

    /// Maximum r/w speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps: Option<i64>,

    /// Maximum length of I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_max_length: Option<u64>,

    /// Maximum read speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_rd: Option<i64>,

    /// Maximum length of read I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_rd_max_length: Option<u64>,

    /// Maximum write speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_wr: Option<i64>,

    /// Maximum length of write I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_wr_max_length: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<PveQmIdeCache>,

    /// Controls whether to detect and try to optimize writes of zeroes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect_zeroes: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discard: Option<PveQmIdeDiscard>,

    /// The drive's backing volume.
    pub file: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<PveQmIdeFormat>,

    /// Create a new disk, importing from this source (volume ID or absolute
    /// path). When an absolute path is specified, it's up to you to ensure that
    /// the source is not actively used by another process during the import!
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "import-from")]
    pub import_from: Option<String>,

    /// Maximum r/w I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops: Option<i64>,

    /// Maximum unthrottled r/w I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_max: Option<i64>,

    /// Maximum length of I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_max_length: Option<u64>,

    /// Maximum read I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd: Option<i64>,

    /// Maximum unthrottled read I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd_max: Option<i64>,

    /// Maximum length of read I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd_max_length: Option<u64>,

    /// Maximum write I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr: Option<i64>,

    /// Maximum unthrottled write I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr_max: Option<i64>,

    /// Maximum length of write I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr_max_length: Option<u64>,

    /// Maximum r/w speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps: Option<f64>,

    /// Maximum unthrottled r/w pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_max: Option<f64>,

    /// Maximum read speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_rd: Option<f64>,

    /// Maximum unthrottled read pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_rd_max: Option<f64>,

    /// Maximum write speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_wr: Option<f64>,

    /// Maximum unthrottled write pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_wr_max: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<PveQmIdeMedia>,

    /// The drive's reported model name, url-encoded, up to 40 bytes long.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Whether the drive should considered for replication jobs.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicate: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerror: Option<PveQmIdeRerror>,

    /// The drive's reported serial number, url-encoded, up to 20 bytes long.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,

    /// Mark this locally-managed volume as available on all nodes
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared: Option<bool>,

    /// Disk size. This is purely informational and has no effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    /// Controls qemu's snapshot mode feature. If activated, changes made to the
    /// disk are temporary and will be discarded when the VM is shutdown.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<bool>,

    /// Whether to expose this drive as an SSD, rather than a rotational hard
    /// disk.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssd: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub werror: Option<PveQmIdeWerror>,

    /// The drive's worldwide name, encoded as 16 bytes hex string, prefixed by
    /// '0x'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wwn: Option<String>,
}

const_regex! {

UPDATE_QEMU_CONFIG_SATA_SERIAL_RE = r##"^[-%a-zA-Z0-9_.!~*'()]*$"##;
UPDATE_QEMU_CONFIG_SATA_SIZE_RE = r##"^(\d+(\.\d+)?)([KMGT])?$"##;

}

#[test]
fn test_regex_compilation_40() {
    use regex::Regex;
    let _: &Regex = &UPDATE_QEMU_CONFIG_SATA_SERIAL_RE;
    let _: &Regex = &UPDATE_QEMU_CONFIG_SATA_SIZE_RE;
}
#[api(
    default_key: "file",
    properties: {
        aio: {
            optional: true,
            type: PveQmIdeAio,
        },
        backup: {
            default: false,
            optional: true,
        },
        bps: {
            optional: true,
            type: Integer,
        },
        bps_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        bps_rd: {
            optional: true,
            type: Integer,
        },
        bps_rd_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        bps_wr: {
            optional: true,
            type: Integer,
        },
        bps_wr_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        cache: {
            optional: true,
            type: PveQmIdeCache,
        },
        detect_zeroes: {
            default: false,
            optional: true,
        },
        discard: {
            optional: true,
            type: PveQmIdeDiscard,
        },
        file: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_volume_id_or_qm_path),
            type: String,
        },
        format: {
            optional: true,
            type: PveQmIdeFormat,
        },
        "import-from": {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_volume_id_or_absolute_path),
            optional: true,
            type: String,
        },
        iops: {
            optional: true,
            type: Integer,
        },
        iops_max: {
            optional: true,
            type: Integer,
        },
        iops_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iops_rd: {
            optional: true,
            type: Integer,
        },
        iops_rd_max: {
            optional: true,
            type: Integer,
        },
        iops_rd_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iops_wr: {
            optional: true,
            type: Integer,
        },
        iops_wr_max: {
            optional: true,
            type: Integer,
        },
        iops_wr_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        media: {
            optional: true,
            type: PveQmIdeMedia,
        },
        replicate: {
            default: true,
            optional: true,
        },
        rerror: {
            optional: true,
            type: PveQmIdeRerror,
        },
        serial: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_SATA_SERIAL_RE),
            max_length: 60,
            optional: true,
            type: String,
        },
        shared: {
            default: false,
            optional: true,
        },
        size: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_SATA_SIZE_RE),
            optional: true,
            type: String,
        },
        snapshot: {
            default: false,
            optional: true,
        },
        ssd: {
            default: false,
            optional: true,
        },
        werror: {
            optional: true,
            type: PveQmIdeWerror,
        },
        wwn: {
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct UpdateQemuConfigSata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aio: Option<PveQmIdeAio>,

    /// Whether the drive should be included when making backups.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<bool>,

    /// Maximum r/w speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps: Option<i64>,

    /// Maximum length of I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_max_length: Option<u64>,

    /// Maximum read speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_rd: Option<i64>,

    /// Maximum length of read I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_rd_max_length: Option<u64>,

    /// Maximum write speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_wr: Option<i64>,

    /// Maximum length of write I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_wr_max_length: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<PveQmIdeCache>,

    /// Controls whether to detect and try to optimize writes of zeroes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect_zeroes: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discard: Option<PveQmIdeDiscard>,

    /// The drive's backing volume.
    pub file: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<PveQmIdeFormat>,

    /// Create a new disk, importing from this source (volume ID or absolute
    /// path). When an absolute path is specified, it's up to you to ensure that
    /// the source is not actively used by another process during the import!
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "import-from")]
    pub import_from: Option<String>,

    /// Maximum r/w I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops: Option<i64>,

    /// Maximum unthrottled r/w I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_max: Option<i64>,

    /// Maximum length of I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_max_length: Option<u64>,

    /// Maximum read I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd: Option<i64>,

    /// Maximum unthrottled read I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd_max: Option<i64>,

    /// Maximum length of read I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd_max_length: Option<u64>,

    /// Maximum write I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr: Option<i64>,

    /// Maximum unthrottled write I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr_max: Option<i64>,

    /// Maximum length of write I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr_max_length: Option<u64>,

    /// Maximum r/w speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps: Option<f64>,

    /// Maximum unthrottled r/w pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_max: Option<f64>,

    /// Maximum read speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_rd: Option<f64>,

    /// Maximum unthrottled read pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_rd_max: Option<f64>,

    /// Maximum write speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_wr: Option<f64>,

    /// Maximum unthrottled write pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_wr_max: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<PveQmIdeMedia>,

    /// Whether the drive should considered for replication jobs.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicate: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerror: Option<PveQmIdeRerror>,

    /// The drive's reported serial number, url-encoded, up to 20 bytes long.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,

    /// Mark this locally-managed volume as available on all nodes
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared: Option<bool>,

    /// Disk size. This is purely informational and has no effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    /// Controls qemu's snapshot mode feature. If activated, changes made to the
    /// disk are temporary and will be discarded when the VM is shutdown.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<bool>,

    /// Whether to expose this drive as an SSD, rather than a rotational hard
    /// disk.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssd: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub werror: Option<PveQmIdeWerror>,

    /// The drive's worldwide name, encoded as 16 bytes hex string, prefixed by
    /// '0x'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wwn: Option<String>,
}

const_regex! {

UPDATE_QEMU_CONFIG_SCSI_SERIAL_RE = r##"^[-%a-zA-Z0-9_.!~*'()]*$"##;
UPDATE_QEMU_CONFIG_SCSI_SIZE_RE = r##"^(\d+(\.\d+)?)([KMGT])?$"##;

}

#[test]
fn test_regex_compilation_41() {
    use regex::Regex;
    let _: &Regex = &UPDATE_QEMU_CONFIG_SCSI_SERIAL_RE;
    let _: &Regex = &UPDATE_QEMU_CONFIG_SCSI_SIZE_RE;
}
#[api(
    default_key: "file",
    properties: {
        aio: {
            optional: true,
            type: PveQmIdeAio,
        },
        backup: {
            default: false,
            optional: true,
        },
        bps: {
            optional: true,
            type: Integer,
        },
        bps_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        bps_rd: {
            optional: true,
            type: Integer,
        },
        bps_rd_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        bps_wr: {
            optional: true,
            type: Integer,
        },
        bps_wr_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        cache: {
            optional: true,
            type: PveQmIdeCache,
        },
        detect_zeroes: {
            default: false,
            optional: true,
        },
        discard: {
            optional: true,
            type: PveQmIdeDiscard,
        },
        file: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_volume_id_or_qm_path),
            type: String,
        },
        format: {
            optional: true,
            type: PveQmIdeFormat,
        },
        "import-from": {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_volume_id_or_absolute_path),
            optional: true,
            type: String,
        },
        iops: {
            optional: true,
            type: Integer,
        },
        iops_max: {
            optional: true,
            type: Integer,
        },
        iops_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iops_rd: {
            optional: true,
            type: Integer,
        },
        iops_rd_max: {
            optional: true,
            type: Integer,
        },
        iops_rd_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iops_wr: {
            optional: true,
            type: Integer,
        },
        iops_wr_max: {
            optional: true,
            type: Integer,
        },
        iops_wr_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iothread: {
            default: false,
            optional: true,
        },
        media: {
            optional: true,
            type: PveQmIdeMedia,
        },
        product: {
            optional: true,
            type: String,
        },
        queues: {
            minimum: 2,
            optional: true,
            type: Integer,
        },
        replicate: {
            default: true,
            optional: true,
        },
        rerror: {
            optional: true,
            type: PveQmIdeRerror,
        },
        ro: {
            default: false,
            optional: true,
        },
        scsiblock: {
            default: false,
            optional: true,
        },
        serial: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_SCSI_SERIAL_RE),
            max_length: 60,
            optional: true,
            type: String,
        },
        shared: {
            default: false,
            optional: true,
        },
        size: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_SCSI_SIZE_RE),
            optional: true,
            type: String,
        },
        snapshot: {
            default: false,
            optional: true,
        },
        ssd: {
            default: false,
            optional: true,
        },
        vendor: {
            optional: true,
            type: String,
        },
        werror: {
            optional: true,
            type: PveQmIdeWerror,
        },
        wwn: {
            optional: true,
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct UpdateQemuConfigScsi {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aio: Option<PveQmIdeAio>,

    /// Whether the drive should be included when making backups.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<bool>,

    /// Maximum r/w speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps: Option<i64>,

    /// Maximum length of I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_max_length: Option<u64>,

    /// Maximum read speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_rd: Option<i64>,

    /// Maximum length of read I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_rd_max_length: Option<u64>,

    /// Maximum write speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_wr: Option<i64>,

    /// Maximum length of write I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_wr_max_length: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<PveQmIdeCache>,

    /// Controls whether to detect and try to optimize writes of zeroes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect_zeroes: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discard: Option<PveQmIdeDiscard>,

    /// The drive's backing volume.
    pub file: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<PveQmIdeFormat>,

    /// Create a new disk, importing from this source (volume ID or absolute
    /// path). When an absolute path is specified, it's up to you to ensure that
    /// the source is not actively used by another process during the import!
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "import-from")]
    pub import_from: Option<String>,

    /// Maximum r/w I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops: Option<i64>,

    /// Maximum unthrottled r/w I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_max: Option<i64>,

    /// Maximum length of I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_max_length: Option<u64>,

    /// Maximum read I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd: Option<i64>,

    /// Maximum unthrottled read I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd_max: Option<i64>,

    /// Maximum length of read I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd_max_length: Option<u64>,

    /// Maximum write I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr: Option<i64>,

    /// Maximum unthrottled write I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr_max: Option<i64>,

    /// Maximum length of write I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr_max_length: Option<u64>,

    /// Whether to use iothreads for this drive
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iothread: Option<bool>,

    /// Maximum r/w speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps: Option<f64>,

    /// Maximum unthrottled r/w pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_max: Option<f64>,

    /// Maximum read speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_rd: Option<f64>,

    /// Maximum unthrottled read pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_rd_max: Option<f64>,

    /// Maximum write speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_wr: Option<f64>,

    /// Maximum unthrottled write pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_wr_max: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<PveQmIdeMedia>,

    /// The drive's product name, up to 16 bytes long.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,

    /// Number of queues.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queues: Option<u64>,

    /// Whether the drive should considered for replication jobs.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicate: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerror: Option<PveQmIdeRerror>,

    /// Whether the drive is read-only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ro: Option<bool>,

    /// whether to use scsi-block for full passthrough of host block device
    ///
    /// WARNING: can lead to I/O errors in combination with low memory or high
    /// memory fragmentation on host
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scsiblock: Option<bool>,

    /// The drive's reported serial number, url-encoded, up to 20 bytes long.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,

    /// Mark this locally-managed volume as available on all nodes
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared: Option<bool>,

    /// Disk size. This is purely informational and has no effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    /// Controls qemu's snapshot mode feature. If activated, changes made to the
    /// disk are temporary and will be discarded when the VM is shutdown.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<bool>,

    /// Whether to expose this drive as an SSD, rather than a rotational hard
    /// disk.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssd: Option<bool>,

    /// The drive's vendor name, up to 8 bytes long.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub werror: Option<PveQmIdeWerror>,

    /// The drive's worldwide name, encoded as 16 bytes hex string, prefixed by
    /// '0x'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wwn: Option<String>,
}

const_regex! {

UPDATE_QEMU_CONFIG_TPMSTATE0_SIZE_RE = r##"^(\d+(\.\d+)?)([KMGT])?$"##;

}

#[test]
fn test_regex_compilation_42() {
    use regex::Regex;
    let _: &Regex = &UPDATE_QEMU_CONFIG_TPMSTATE0_SIZE_RE;
}
#[api(
    default_key: "file",
    properties: {
        file: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_volume_id_or_qm_path),
            type: String,
        },
        "import-from": {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_volume_id_or_absolute_path),
            optional: true,
            type: String,
        },
        size: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_TPMSTATE0_SIZE_RE),
            optional: true,
            type: String,
        },
        version: {
            optional: true,
            type: QemuConfigTpmstate0Version,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct UpdateQemuConfigTpmstate0 {
    /// The drive's backing volume.
    pub file: String,

    /// Create a new disk, importing from this source (volume ID or absolute
    /// path). When an absolute path is specified, it's up to you to ensure that
    /// the source is not actively used by another process during the import!
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "import-from")]
    pub import_from: Option<String>,

    /// Disk size. This is purely informational and has no effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<QemuConfigTpmstate0Version>,
}

const_regex! {

UPDATE_QEMU_CONFIG_VIRTIO_SERIAL_RE = r##"^[-%a-zA-Z0-9_.!~*'()]*$"##;
UPDATE_QEMU_CONFIG_VIRTIO_SIZE_RE = r##"^(\d+(\.\d+)?)([KMGT])?$"##;

}

#[test]
fn test_regex_compilation_43() {
    use regex::Regex;
    let _: &Regex = &UPDATE_QEMU_CONFIG_VIRTIO_SERIAL_RE;
    let _: &Regex = &UPDATE_QEMU_CONFIG_VIRTIO_SIZE_RE;
}
#[api(
    default_key: "file",
    properties: {
        aio: {
            optional: true,
            type: PveQmIdeAio,
        },
        backup: {
            default: false,
            optional: true,
        },
        bps: {
            optional: true,
            type: Integer,
        },
        bps_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        bps_rd: {
            optional: true,
            type: Integer,
        },
        bps_rd_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        bps_wr: {
            optional: true,
            type: Integer,
        },
        bps_wr_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        cache: {
            optional: true,
            type: PveQmIdeCache,
        },
        detect_zeroes: {
            default: false,
            optional: true,
        },
        discard: {
            optional: true,
            type: PveQmIdeDiscard,
        },
        file: {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_volume_id_or_qm_path),
            type: String,
        },
        format: {
            optional: true,
            type: PveQmIdeFormat,
        },
        "import-from": {
            format: &ApiStringFormat::VerifyFn(verifiers::verify_pve_volume_id_or_absolute_path),
            optional: true,
            type: String,
        },
        iops: {
            optional: true,
            type: Integer,
        },
        iops_max: {
            optional: true,
            type: Integer,
        },
        iops_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iops_rd: {
            optional: true,
            type: Integer,
        },
        iops_rd_max: {
            optional: true,
            type: Integer,
        },
        iops_rd_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iops_wr: {
            optional: true,
            type: Integer,
        },
        iops_wr_max: {
            optional: true,
            type: Integer,
        },
        iops_wr_max_length: {
            minimum: 1,
            optional: true,
            type: Integer,
        },
        iothread: {
            default: false,
            optional: true,
        },
        media: {
            optional: true,
            type: PveQmIdeMedia,
        },
        replicate: {
            default: true,
            optional: true,
        },
        rerror: {
            optional: true,
            type: PveQmIdeRerror,
        },
        ro: {
            default: false,
            optional: true,
        },
        serial: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_VIRTIO_SERIAL_RE),
            max_length: 60,
            optional: true,
            type: String,
        },
        shared: {
            default: false,
            optional: true,
        },
        size: {
            format: &ApiStringFormat::Pattern(&UPDATE_QEMU_CONFIG_VIRTIO_SIZE_RE),
            optional: true,
            type: String,
        },
        snapshot: {
            default: false,
            optional: true,
        },
        werror: {
            optional: true,
            type: PveQmIdeWerror,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct UpdateQemuConfigVirtio {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aio: Option<PveQmIdeAio>,

    /// Whether the drive should be included when making backups.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<bool>,

    /// Maximum r/w speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps: Option<i64>,

    /// Maximum length of I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_max_length: Option<u64>,

    /// Maximum read speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_rd: Option<i64>,

    /// Maximum length of read I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_rd_max_length: Option<u64>,

    /// Maximum write speed in bytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_wr: Option<i64>,

    /// Maximum length of write I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bps_wr_max_length: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<PveQmIdeCache>,

    /// Controls whether to detect and try to optimize writes of zeroes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect_zeroes: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discard: Option<PveQmIdeDiscard>,

    /// The drive's backing volume.
    pub file: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<PveQmIdeFormat>,

    /// Create a new disk, importing from this source (volume ID or absolute
    /// path). When an absolute path is specified, it's up to you to ensure that
    /// the source is not actively used by another process during the import!
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "import-from")]
    pub import_from: Option<String>,

    /// Maximum r/w I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops: Option<i64>,

    /// Maximum unthrottled r/w I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_max: Option<i64>,

    /// Maximum length of I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_max_length: Option<u64>,

    /// Maximum read I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd: Option<i64>,

    /// Maximum unthrottled read I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd_max: Option<i64>,

    /// Maximum length of read I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_rd_max_length: Option<u64>,

    /// Maximum write I/O in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr: Option<i64>,

    /// Maximum unthrottled write I/O pool in operations per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr_max: Option<i64>,

    /// Maximum length of write I/O bursts in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iops_wr_max_length: Option<u64>,

    /// Whether to use iothreads for this drive
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iothread: Option<bool>,

    /// Maximum r/w speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps: Option<f64>,

    /// Maximum unthrottled r/w pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_max: Option<f64>,

    /// Maximum read speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_rd: Option<f64>,

    /// Maximum unthrottled read pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_rd_max: Option<f64>,

    /// Maximum write speed in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_wr: Option<f64>,

    /// Maximum unthrottled write pool in megabytes per second.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps_wr_max: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<PveQmIdeMedia>,

    /// Whether the drive should considered for replication jobs.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicate: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerror: Option<PveQmIdeRerror>,

    /// Whether the drive is read-only.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ro: Option<bool>,

    /// The drive's reported serial number, url-encoded, up to 20 bytes long.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,

    /// Mark this locally-managed volume as available on all nodes
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared: Option<bool>,

    /// Disk size. This is purely informational and has no effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    /// Controls qemu's snapshot mode feature. If activated, changes made to the
    /// disk are temporary and will be discarded when the VM is shutdown.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub werror: Option<PveQmIdeWerror>,
}

#[api(
    properties: {
        console: {
            optional: true,
            type: VersionResponseConsole,
        },
        release: {
            type: String,
        },
        repoid: {
            type: String,
        },
        version: {
            type: String,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct VersionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub console: Option<VersionResponseConsole>,

    /// The current Proxmox VE point release in `x.y` format.
    pub release: String,

    /// The short git revision from which this version was build.
    pub repoid: String,

    /// The full pve-manager package version of this node.
    pub version: String,
}

#[api]
/// The default console viewer to use.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum VersionResponseConsole {
    #[serde(rename = "applet")]
    /// applet.
    Applet,
    #[serde(rename = "vv")]
    /// vv.
    Vv,
    #[serde(rename = "html5")]
    /// html5.
    Html5,
    #[serde(rename = "xtermjs")]
    /// xtermjs.
    Xtermjs,
    /// Unknown variants for forward compatibility.
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}
serde_plain::derive_display_from_serialize!(VersionResponseConsole);
serde_plain::derive_fromstr_from_deserialize!(VersionResponseConsole);

#[api(
    properties: {
        diskread: {
            optional: true,
            type: Integer,
        },
        diskwrite: {
            optional: true,
            type: Integer,
        },
        lock: {
            optional: true,
            type: String,
        },
        maxdisk: {
            optional: true,
            type: Integer,
        },
        maxmem: {
            optional: true,
            type: Integer,
        },
        mem: {
            optional: true,
            type: Integer,
        },
        memhost: {
            optional: true,
            type: Integer,
        },
        name: {
            optional: true,
            type: String,
        },
        netin: {
            optional: true,
            type: Integer,
        },
        netout: {
            optional: true,
            type: Integer,
        },
        pid: {
            optional: true,
            type: Integer,
        },
        qmpstatus: {
            optional: true,
            type: String,
        },
        "running-machine": {
            optional: true,
            type: String,
        },
        "running-qemu": {
            optional: true,
            type: String,
        },
        serial: {
            default: false,
            optional: true,
        },
        status: {
            type: IsRunning,
        },
        tags: {
            optional: true,
            type: String,
        },
        template: {
            default: false,
            optional: true,
        },
        uptime: {
            optional: true,
            type: Integer,
        },
        vmid: {
            maximum: 999999999,
            minimum: 100,
            type: Integer,
        },
    },
)]
/// Object.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct VmEntry {
    /// Current CPU usage.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<f64>,

    /// Maximum usable CPUs.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpus: Option<f64>,

    /// The amount of bytes the guest read from it's block devices since the
    /// guest was started. (Note: This info is not available for all storage
    /// types.)
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diskread: Option<i64>,

    /// The amount of bytes the guest wrote from it's block devices since the
    /// guest was started. (Note: This info is not available for all storage
    /// types.)
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diskwrite: Option<i64>,

    /// The current config lock, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<String>,

    /// Root disk size in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxdisk: Option<i64>,

    /// Maximum memory in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxmem: Option<i64>,

    /// Currently used memory in bytes.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mem: Option<i64>,

    /// Current memory usage on the host.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memhost: Option<i64>,

    /// VM (host)name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// The amount of traffic in bytes that was sent to the guest over the
    /// network since it was started.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netin: Option<i64>,

    /// The amount of traffic in bytes that was sent from the guest over the
    /// network since it was started.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netout: Option<i64>,

    /// PID of the QEMU process, if the VM is running.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<i64>,

    /// CPU Full pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressurecpufull: Option<f64>,

    /// CPU Some pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressurecpusome: Option<f64>,

    /// IO Full pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressureiofull: Option<f64>,

    /// IO Some pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressureiosome: Option<f64>,

    /// Memory Full pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressurememoryfull: Option<f64>,

    /// Memory Some pressure stall average over the last 10 seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_f64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressurememorysome: Option<f64>,

    /// VM run state from the 'query-status' QMP monitor command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qmpstatus: Option<String>,

    /// The currently running machine type (if running).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "running-machine")]
    pub running_machine: Option<String>,

    /// The QEMU version the VM is currently using (if running).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "running-qemu")]
    pub running_qemu: Option<String>,

    /// Guest has serial device configured.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<bool>,

    pub status: IsRunning,

    /// The current configured tags, if any
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,

    /// Determines if the guest is a template.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_bool")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<bool>,

    /// Uptime in seconds.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_i64")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime: Option<i64>,

    /// The (unique) ID of the VM.
    #[serde(deserialize_with = "proxmox_serde::perl::deserialize_u32")]
    pub vmid: u32,
}
