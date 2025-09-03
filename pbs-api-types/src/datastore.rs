use std::convert::{AsRef, TryFrom};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::LazyLock;

use anyhow::{bail, format_err, Error};
use const_format::concatcp;
use serde::{Deserialize, Serialize};

use proxmox_human_byte::HumanByte;
use proxmox_schema::{
    api, const_regex, ApiStringFormat, ApiType, ArraySchema, EnumEntry, IntegerSchema, ReturnType,
    Schema, StringSchema, Updater, UpdaterType,
};

use crate::{
    Authid, CryptMode, Fingerprint, GroupFilter, MaintenanceMode, MaintenanceType, Userid,
    BACKUP_ID_RE, BACKUP_NS_RE, BACKUP_TIME_RE, BACKUP_TYPE_RE, DATASTORE_NOTIFY_STRING_SCHEMA,
    GC_SCHEDULE_SCHEMA, GROUP_OR_SNAPSHOT_PATH_REGEX_STR, PROXMOX_SAFE_ID_FORMAT,
    PROXMOX_SAFE_ID_REGEX_STR, PRUNE_SCHEDULE_SCHEMA, SHA256_HEX_REGEX, SINGLE_LINE_COMMENT_SCHEMA,
    SNAPSHOT_PATH_REGEX_STR, UPID,
};

const_regex! {
    pub BACKUP_NAMESPACE_REGEX = concatcp!(r"^", BACKUP_NS_RE, r"$");

    pub BACKUP_TYPE_REGEX = concatcp!(r"^(", BACKUP_TYPE_RE, r")$");

    pub BACKUP_ID_REGEX = concatcp!(r"^", BACKUP_ID_RE, r"$");

    pub BACKUP_DATE_REGEX = concatcp!(r"^", BACKUP_TIME_RE ,r"$");

    pub GROUP_PATH_REGEX = concatcp!(
        r"^(", BACKUP_TYPE_RE, ")/",
        r"(", BACKUP_ID_RE, r")$",
    );

    pub BACKUP_FILE_REGEX = r"^.*\.([fd]idx|blob)$";

    pub SNAPSHOT_PATH_REGEX = concatcp!(r"^", SNAPSHOT_PATH_REGEX_STR, r"$");
    pub GROUP_OR_SNAPSHOT_PATH_REGEX = concatcp!(r"^", GROUP_OR_SNAPSHOT_PATH_REGEX_STR, r"$");

    pub DATASTORE_MAP_REGEX = concatcp!(r"^(?:", PROXMOX_SAFE_ID_REGEX_STR, r"=)?", PROXMOX_SAFE_ID_REGEX_STR, r"$");
}

pub const CHUNK_DIGEST_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&SHA256_HEX_REGEX);

pub const DATASTORE_DIR_NAME_SCHEMA: Schema = StringSchema::new("Either the absolute path to the datastore directory, or a relative on-device path for removable datastores.")
    .min_length(1)
    .max_length(4096)
    .schema();

pub const BACKUP_ARCHIVE_NAME_SCHEMA: Schema = StringSchema::new("Backup archive name.")
    .format(&PROXMOX_SAFE_ID_FORMAT)
    .schema();

pub const BACKUP_ID_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&BACKUP_ID_REGEX);
pub const BACKUP_GROUP_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&GROUP_PATH_REGEX);
pub const BACKUP_NAMESPACE_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&BACKUP_NAMESPACE_REGEX);

pub const BACKUP_ID_SCHEMA: Schema = StringSchema::new("Backup ID.")
    .format(&BACKUP_ID_FORMAT)
    .schema();

pub const BACKUP_TYPE_SCHEMA: Schema = StringSchema::new("Backup type.")
    .format(&ApiStringFormat::Enum(&[
        EnumEntry::new("vm", "Virtual Machine Backup"),
        EnumEntry::new("ct", "Container Backup"),
        EnumEntry::new("host", "Host Backup"),
    ]))
    .schema();

pub const BACKUP_TIME_SCHEMA: Schema = IntegerSchema::new("Backup time (Unix epoch.)")
    .minimum(1)
    .schema();

pub const BACKUP_GROUP_SCHEMA: Schema = StringSchema::new("Backup Group")
    .format(&BACKUP_GROUP_FORMAT)
    .schema();

/// The maximal, inclusive depth for namespaces from the root ns downwards
///
/// The datastore root name space is at depth zero (0), so we have in total eight (8) levels
pub const MAX_NAMESPACE_DEPTH: usize = 7;
pub const MAX_BACKUP_NAMESPACE_LENGTH: usize = 32 * 8; // 256
pub const BACKUP_NAMESPACE_SCHEMA: Schema = StringSchema::new("Namespace.")
    .format(&BACKUP_NAMESPACE_FORMAT)
    .max_length(MAX_BACKUP_NAMESPACE_LENGTH) // 256
    .schema();

pub const NS_MAX_DEPTH_SCHEMA: Schema =
    IntegerSchema::new("How many levels of namespaces should be operated on (0 == no recursion)")
        .minimum(0)
        .maximum(MAX_NAMESPACE_DEPTH as i64)
        .default(MAX_NAMESPACE_DEPTH as i64)
        .schema();

pub const NS_MAX_DEPTH_REDUCED_SCHEMA: Schema =
IntegerSchema::new("How many levels of namespaces should be operated on (0 == no recursion, empty == automatic full recursion, namespace depths reduce maximum allowed value)")
    .minimum(0)
    .maximum(MAX_NAMESPACE_DEPTH as i64)
    .schema();

pub const DATASTORE_SCHEMA: Schema = StringSchema::new("Datastore name.")
    .format(&PROXMOX_SAFE_ID_FORMAT)
    .min_length(3)
    .max_length(32)
    .schema();

pub const CHUNK_DIGEST_SCHEMA: Schema = StringSchema::new("Chunk digest (SHA256).")
    .format(&CHUNK_DIGEST_FORMAT)
    .schema();

pub const DATASTORE_MAP_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&DATASTORE_MAP_REGEX);

pub const DATASTORE_MAP_SCHEMA: Schema = StringSchema::new("Datastore mapping.")
    .format(&DATASTORE_MAP_FORMAT)
    .min_length(3)
    .max_length(65)
    .type_text("(<source>=)?<target>")
    .schema();

pub const DATASTORE_MAP_ARRAY_SCHEMA: Schema =
    ArraySchema::new("Datastore mapping list.", &DATASTORE_MAP_SCHEMA).schema();

pub const DATASTORE_MAP_LIST_SCHEMA: Schema = StringSchema::new(
    "A list of Datastore mappings (or single datastore), comma separated. \
    For example 'a=b,e' maps the source datastore 'a' to target 'b and \
    all other sources to the default 'e'. If no default is given, only the \
    specified sources are mapped.",
)
.format(&ApiStringFormat::PropertyString(
    &DATASTORE_MAP_ARRAY_SCHEMA,
))
.schema();

pub const PRUNE_SCHEMA_KEEP_DAILY: Schema = IntegerSchema::new("Number of daily backups to keep.")
    .minimum(1)
    .schema();

pub const PRUNE_SCHEMA_KEEP_HOURLY: Schema =
    IntegerSchema::new("Number of hourly backups to keep.")
        .minimum(1)
        .schema();

pub const PRUNE_SCHEMA_KEEP_LAST: Schema = IntegerSchema::new("Number of backups to keep.")
    .minimum(1)
    .schema();

pub const PRUNE_SCHEMA_KEEP_MONTHLY: Schema =
    IntegerSchema::new("Number of monthly backups to keep.")
        .minimum(1)
        .schema();

pub const PRUNE_SCHEMA_KEEP_WEEKLY: Schema =
    IntegerSchema::new("Number of weekly backups to keep.")
        .minimum(1)
        .schema();

pub const PRUNE_SCHEMA_KEEP_YEARLY: Schema =
    IntegerSchema::new("Number of yearly backups to keep.")
        .minimum(1)
        .schema();

/// Base directory where datastores are mounted
pub const DATASTORE_MOUNT_DIR: &str = "/mnt/datastore";

#[api]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// The order to sort chunks by
pub enum ChunkOrder {
    /// Iterate chunks in the index order
    None,
    /// Iterate chunks in inode order
    #[default]
    Inode,
}

#[api]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// Current mounting status of a datastore, useful for removable datastores.
pub enum DataStoreMountStatus {
    /// Removable datastore is currently mounted correctly.
    Mounted,
    /// Removable datastore is currebtly not mounted.
    NotMounted,
    /// Datastore is not removable, so there is no mount status.
    #[default]
    NonRemovable,
}

#[api]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// The level of syncing that is done when writing into a datastore.
pub enum DatastoreFSyncLevel {
    /// No special fsync or syncfs calls are triggered. The system default dirty write back
    /// mechanism ensures that data gets is flushed eventually via the `dirty_writeback_centisecs`
    /// and `dirty_expire_centisecs` kernel sysctls, defaulting to ~ 30s.
    ///
    /// This mode provides generally the best performance, as all write back can happen async,
    /// which reduces IO pressure.
    /// But it may cause losing data on powerloss or system crash without any uninterruptible power
    /// supply.
    None,
    /// Triggers a fsync after writing any chunk on the datastore. While this can slow down
    /// backups significantly, depending on the underlying file system and storage used, it
    /// will ensure fine-grained consistency. Depending on the exact setup, there might be no
    /// benefits over the file system level sync, so if the setup allows it, you should prefer
    /// that one. Despite the possible negative impact in performance, it's the most consistent
    /// mode.
    File,
    /// Trigger a filesystem wide sync after all backup data got written but before finishing the
    /// task. This allows that every finished backup is fully written back to storage
    /// while reducing the impact on many file systems in contrast to the file level sync.
    /// Depending on the setup, it might have a negative impact on unrelated write operations
    /// of the underlying filesystem, but it is generally a good compromise between performance
    /// and consistency.
    #[default]
    Filesystem,
}

pub const GC_ATIME_CUTOFF_SCHEMA: Schema = IntegerSchema::new(
    "Cutoff (in minutes) for chunk cleanup atime check in garbage collection phase 2 \
        (default 24h 5m)",
)
.minimum(1) // safety margin for kernel timestamp granularity, but stay within minute range
.maximum(2 * 24 * 60)
.default(24 * 60 + 5)
.schema();

pub const GC_CACHE_CAPACITY_SCHEMA: Schema =
    IntegerSchema::new("Garbage collection chunk digest cache capacity")
        .minimum(0)
        .maximum(8 * 1024 * 1024)
        .default(1024 * 1024)
        .schema();

#[api(
    properties: {
        "chunk-order": {
            type: ChunkOrder,
            optional: true,
        },
        "gc-atime-safety-check": {
            description:
                "Check filesystem atime updates are honored during store creation and garbage \
                collection",
            optional: true,
            default: true,
            type: bool,
        },
        "gc-atime-cutoff": {
            schema: GC_ATIME_CUTOFF_SCHEMA,
            optional: true,
        },
        "gc-cache-capacity": {
            schema: GC_CACHE_CAPACITY_SCHEMA,
            optional: true,
        },
    },
)]
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
/// Datastore tuning options
pub struct DatastoreTuning {
    /// Iterate chunks in this order
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_order: Option<ChunkOrder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_level: Option<DatastoreFSyncLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gc_atime_safety_check: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gc_atime_cutoff: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gc_cache_capacity: Option<usize>,
}

pub const DATASTORE_TUNING_STRING_SCHEMA: Schema = StringSchema::new("Datastore tuning options")
    .format(&ApiStringFormat::PropertyString(
        &DatastoreTuning::API_SCHEMA,
    ))
    .schema();

#[api]
#[derive(Copy, Clone, Default, Deserialize, Serialize, Updater, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Datastore backend type
pub enum DatastoreBackendType {
    /// Local filesystem
    #[default]
    Filesystem,
    /// S3 object store
    S3,
}
serde_plain::derive_display_from_serialize!(DatastoreBackendType);
serde_plain::derive_fromstr_from_deserialize!(DatastoreBackendType);

#[api(
    properties: {
        type: {
            type: DatastoreBackendType,
            optional: true,
        },
        client: {
            schema: proxmox_s3_client::S3_CLIENT_ID_SCHEMA,
            optional: true,
        },
        bucket: {
            schema: proxmox_s3_client::S3_BUCKET_NAME_SCHEMA,
            optional: true,
        },
        "max-cache-size": {
            type: HumanByte,
            optional: true,
        }
    },
    default_key: "type",
)]
#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Datastore backend config
pub struct DatastoreBackendConfig {
    /// backend type
    #[serde(rename = "type")]
    pub ty: Option<DatastoreBackendType>,
    /// s3 client id
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    /// s3 bucket name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bucket: Option<String>,
    /// maximum cache size for local datastore LRU cache
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_cache_size: Option<HumanByte>,
}

pub const DATASTORE_BACKEND_CONFIG_STRING_SCHEMA: Schema =
    StringSchema::new("Datastore backend config")
        .format(&ApiStringFormat::VerifyFn(verify_datastore_backend_config))
        .type_text("<backend-config>")
        .schema();

fn verify_datastore_backend_config(input: &str) -> Result<(), Error> {
    DatastoreBackendConfig::from_str(input).map(|_| ())
}

impl FromStr for DatastoreBackendConfig {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let backend_config: DatastoreBackendConfig =
            proxmox_schema::property_string::parse_with_schema(
                s,
                &DatastoreBackendConfig::API_SCHEMA,
            )?;
        let backend_type = backend_config.ty.unwrap_or_default();
        match backend_type {
            DatastoreBackendType::Filesystem => {
                if backend_config.client.is_some() {
                    bail!("additional option client, not allowed for backend type filesystem");
                }
                if backend_config.bucket.is_some() {
                    bail!("additional option bucket, not allowed for backend type filesystem");
                }
                if backend_config.max_cache_size.is_some() {
                    bail!(
                        "additional option max-cache-size, not allowed for backend type filesystem"
                    );
                }
            }
            DatastoreBackendType::S3 => {
                if backend_config.client.is_none() {
                    bail!("missing option client, required for backend type s3");
                }
                if backend_config.bucket.is_none() {
                    bail!("missing option bucket, required for backend type s3");
                }
            }
        }
        Ok(backend_config)
    }
}

#[api(
    properties: {
        name: {
            schema: DATASTORE_SCHEMA,
        },
        path: {
            schema: DATASTORE_DIR_NAME_SCHEMA,
        },
        "notify-user": {
            optional: true,
            type: Userid,
        },
        "notify": {
            optional: true,
            schema: DATASTORE_NOTIFY_STRING_SCHEMA,
        },
        comment: {
            optional: true,
            schema: SINGLE_LINE_COMMENT_SCHEMA,
        },
        "gc-schedule": {
            optional: true,
            schema: GC_SCHEDULE_SCHEMA,
        },
        "prune-schedule": {
            optional: true,
            schema: PRUNE_SCHEDULE_SCHEMA,
        },
        keep: {
            type: crate::KeepOptions,
        },
        "verify-new": {
            description: "If enabled, all new backups will be verified right after completion.",
            optional: true,
            type: bool,
        },
        tuning: {
            optional: true,
            schema: DATASTORE_TUNING_STRING_SCHEMA,
        },
        "maintenance-mode": {
            optional: true,
            format: &ApiStringFormat::PropertyString(&MaintenanceMode::API_SCHEMA),
            type: String,
        },
        "backing-device": {
            description: "The UUID of the filesystem partition for removable datastores.",
            optional: true,
            format: &proxmox_schema::api_types::UUID_FORMAT,
            type: String,
        },
        backend: {
            schema: DATASTORE_BACKEND_CONFIG_STRING_SCHEMA,
            optional: true,
        },
    }
)]
#[derive(Serialize, Deserialize, Updater, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Datastore configuration properties.
pub struct DataStoreConfig {
    #[updater(skip)]
    pub name: String,

    #[updater(skip)]
    pub path: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gc_schedule: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub prune_schedule: Option<String>,

    #[serde(flatten)]
    pub keep: crate::KeepOptions,

    /// If enabled, all backups will be verified right after completion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify_new: Option<bool>,

    /// Send job email notification to this user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify_user: Option<Userid>,

    /// Send notification only for job errors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify: Option<String>,

    /// Opt in to the new notification system
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_mode: Option<NotificationMode>,

    /// Datastore tuning options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tuning: Option<String>,

    /// Maintenance mode, type is either 'offline' or 'read-only', message should be enclosed in "
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maintenance_mode: Option<String>,

    /// The UUID of the device(for removable datastores)
    #[updater(skip)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backing_device: Option<String>,

    /// Backend configuration for datastore
    #[updater(skip)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
}

#[api]
#[derive(Serialize, Deserialize, Updater, Clone, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
/// Configure how notifications for this datastore should be sent.
/// `legacy-sendmail` sends email notifications to the user configured
/// in `notify-user` via the system's `sendmail` executable.
/// `notification-system` emits matchable notification events to the
/// notification system.
pub enum NotificationMode {
    /// Send notifications via the system's sendmail command to the user
    /// configured in `notify-user`
    LegacySendmail,
    /// Emit notification events to the notification system
    #[default]
    NotificationSystem,
}

impl DataStoreConfig {
    pub fn new(name: String, path: String) -> Self {
        Self {
            name,
            path,
            comment: None,
            gc_schedule: None,
            prune_schedule: None,
            keep: Default::default(),
            verify_new: None,
            notify_user: None,
            notify: None,
            notification_mode: None,
            tuning: None,
            maintenance_mode: None,
            backing_device: None,
            backend: None,
        }
    }

    /// Returns the absolute path to the datastore content.
    pub fn absolute_path(&self) -> String {
        if self.backing_device.is_some() {
            format!("{DATASTORE_MOUNT_DIR}/{}", self.name)
        } else {
            self.path.clone()
        }
    }

    pub fn get_maintenance_mode(&self) -> Option<MaintenanceMode> {
        self.maintenance_mode.as_ref().and_then(|str| {
            MaintenanceMode::deserialize(proxmox_schema::de::SchemaDeserializer::new(
                str,
                &MaintenanceMode::API_SCHEMA,
            ))
            .ok()
        })
    }

    pub fn set_maintenance_mode(&mut self, new_mode: Option<MaintenanceMode>) -> Result<(), Error> {
        let current_type = self.get_maintenance_mode().map(|mode| mode.ty);
        let new_type = new_mode.as_ref().map(|mode| mode.ty);

        match current_type {
            Some(MaintenanceType::ReadOnly) => { /* always OK  */ }
            Some(MaintenanceType::Offline) => { /* always OK  */ }
            Some(MaintenanceType::Unmount) => {
                /* used to reset it after failed unmount, or alternative for aborting unmount task */
            }
            Some(MaintenanceType::S3Refresh) => { /* used to reset state after refresh finished */ }
            Some(MaintenanceType::Delete) => {
                match new_type {
                    Some(MaintenanceType::Delete) => { /* allow to delete a deleted storage */ }
                    _ => {
                        bail!("datastore is being deleted")
                    }
                }
            }
            None => { /* always OK  */ }
        }

        let new_mode = match new_mode {
            Some(new_mode) => Some(
                proxmox_schema::property_string::PropertyString::new(new_mode)
                    .to_property_string()?,
            ),
            None => None,
        };

        self.maintenance_mode = new_mode;

        Ok(())
    }

    pub fn ensure_not_nested(&self, stores: &[DataStoreConfig]) -> Result<(), Error> {
        let our_absolute_path = PathBuf::from(self.absolute_path());
        let removable = self.backing_device.is_some();
        for other_store in stores {
            if self == other_store {
                continue;
            };

            // Relative paths must not be nested on the backing device of removable datastores
            if removable && other_store.backing_device == self.backing_device {
                let our_relative_path = Path::new(&self.path);
                let other_relative_path = Path::new(&other_store.path);
                if our_relative_path.starts_with(other_relative_path)
                    || other_relative_path.starts_with(our_relative_path)
                {
                    bail!(
                        "paths on backing device must not be nested - {path:?} already used by '{store}'!",
                        path = other_relative_path,
                        store = other_store.name,
                    );
                }
            }

            // No two datastores should have a nested absolute path
            let other_absolute_path = PathBuf::from(other_store.absolute_path());
            if other_absolute_path.starts_with(&our_absolute_path)
                || our_absolute_path.starts_with(&other_absolute_path)
            {
                bail!(
                    "nested datastores not allowed: '{}' already in {:?}",
                    other_store.name,
                    other_absolute_path,
                );
            }
        }

        Ok(())
    }
}

#[api(
    properties: {
        store: {
            schema: DATASTORE_SCHEMA,
        },
        comment: {
            optional: true,
            schema: SINGLE_LINE_COMMENT_SCHEMA,
        },
        "mount-status": {
            type: DataStoreMountStatus,
        },
        maintenance: {
            optional: true,
            format: &ApiStringFormat::PropertyString(&MaintenanceMode::API_SCHEMA),
            type: String,
        }
    },
)]
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Basic information about a datastore.
pub struct DataStoreListItem {
    pub store: String,
    pub comment: Option<String>,
    #[serde(default)]
    pub mount_status: DataStoreMountStatus,
    /// If the datastore is in maintenance mode, information about it
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maintenance: Option<String>,
}

#[api(
    properties: {
        "filename": {
            schema: BACKUP_ARCHIVE_NAME_SCHEMA,
        },
        "crypt-mode": {
            type: CryptMode,
            optional: true,
        },
    },
)]
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Basic information about archive files inside a backup snapshot.
pub struct BackupContent {
    pub filename: String,
    /// Info if file is encrypted, signed, or neither.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crypt_mode: Option<CryptMode>,
    /// Archive size (from backup manifest).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

#[api()]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// Result of a verify operation.
pub enum VerifyState {
    /// Verification was successful
    Ok,
    /// Verification reported one or more errors
    Failed,
}

#[api(
    properties: {
        upid: {
            type: UPID,
        },
        state: {
            type: VerifyState,
        },
    },
)]
#[derive(Serialize, Deserialize, Clone, PartialEq)]
/// Task properties.
pub struct SnapshotVerifyState {
    /// UPID of the verify task
    pub upid: UPID,
    /// State of the verification. Enum.
    pub state: VerifyState,
}

/// A namespace provides a logical separation between backup groups from different domains
/// (cluster, sites, ...) where uniqueness cannot be guaranteed anymore. It allows users to share a
/// datastore (i.e., one deduplication domain (chunk store)) with multiple (trusted) sites and
/// allows to form a hierarchy, for easier management and avoiding clashes between backup_ids.
///
/// NOTE: Namespaces are a logical boundary only, they do not provide a full secure separation as
/// the chunk store is still shared. So, users whom do not trust each other must not share a
/// datastore.
///
/// Implementation note: The path a namespace resolves to is always prefixed with `/ns` to avoid
/// clashes with backup group IDs and future backup_types and to have a clean separation between
/// the namespace directories and the ones from a backup snapshot.
#[derive(Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, UpdaterType)]
pub struct BackupNamespace {
    /// The namespace subdirectories without the `ns/` intermediate directories.
    inner: Vec<String>,

    /// Cache the total length for efficiency.
    len: usize,
}

impl BackupNamespace {
    /// Returns a root namespace reference.
    pub const fn root() -> Self {
        Self {
            inner: Vec::new(),
            len: 0,
        }
    }

    /// True if this represents the root namespace.
    pub fn is_root(&self) -> bool {
        self.inner.is_empty()
    }

    /// Try to parse a string into a namespace.
    pub fn new(name: &str) -> Result<Self, Error> {
        let mut this = Self::root();

        if name.is_empty() {
            return Ok(this);
        }

        for name in name.split('/') {
            this.push(name.to_string())?;
        }
        Ok(this)
    }

    /// Try to parse a file path string (where each sub-namespace is separated by an `ns`
    /// subdirectory) into a valid namespace.
    pub fn from_path(mut path: &str) -> Result<Self, Error> {
        let mut this = Self::root();
        loop {
            match path.strip_prefix("ns/") {
                Some(next) => match next.find('/') {
                    Some(pos) => {
                        this.push(next[..pos].to_string())?;
                        path = &next[(pos + 1)..];
                    }
                    None => {
                        this.push(next.to_string())?;
                        break;
                    }
                },
                None if !path.is_empty() => {
                    bail!("invalid component in namespace path at {:?}", path);
                }
                None => break,
            }
        }
        Ok(this)
    }

    /// Create a new Namespace attached to parent
    ///
    /// `name` must be a single level namespace ID, that is, no '/' is allowed.
    /// This rule also avoids confusion about the name being a NS or NS-path
    pub fn from_parent_ns(parent: &Self, name: String) -> Result<Self, Error> {
        let mut child = parent.to_owned();
        child.push(name)?;
        Ok(child)
    }

    /// Pop one level off the namespace hierarchy
    pub fn pop(&mut self) -> Option<String> {
        let dropped = self.inner.pop();
        if let Some(ref dropped) = dropped {
            self.len = self.len.saturating_sub(dropped.len() + 1);
        }
        dropped
    }

    /// Get the namespace parent as owned BackupNamespace
    pub fn parent(&self) -> Self {
        if self.is_root() {
            return Self::root();
        }

        let mut parent = self.clone();
        parent.pop();

        parent
    }

    /// Create a new namespace directly from a vec.
    ///
    /// # Safety
    ///
    /// Invalid contents may lead to inaccessible backups.
    pub unsafe fn from_vec_unchecked(components: Vec<String>) -> Self {
        let mut this = Self {
            inner: components,
            len: 0,
        };
        this.recalculate_len();
        this
    }

    /// Recalculate the length.
    fn recalculate_len(&mut self) {
        self.len = self.inner.len().max(1) - 1; // a slash between each component
        for part in &self.inner {
            self.len += part.len();
        }
    }

    /// The hierarchical depth of the namespace, 0 means top-level.
    pub fn depth(&self) -> usize {
        self.inner.len()
    }

    /// The logical name and ID of the namespace.
    pub fn name(&self) -> String {
        self.to_string()
    }

    /// The actual relative backing path of the namespace on the datastore.
    pub fn path(&self) -> PathBuf {
        self.display_as_path().to_string().into()
    }

    /// Get the current namespace length.
    ///
    /// This includes separating slashes, but does not include the `ns/` intermediate directories.
    /// This is not the *path* length, but rather the length that would be produced via
    /// `.to_string()`.
    #[inline]
    pub fn name_len(&self) -> usize {
        self.len
    }

    /// Get the current namespace path length.
    ///
    /// This includes the `ns/` subdirectory strings.
    pub fn path_len(&self) -> usize {
        self.name_len() + 3 * self.inner.len()
    }

    /// Enter a sub-namespace. Fails if nesting would become too deep or the name too long.
    pub fn push(&mut self, subdir: String) -> Result<(), Error> {
        if subdir.contains('/') {
            bail!("namespace component contained a slash");
        }

        self.push_do(subdir)
    }

    /// Assumes `subdir` already does not contain any slashes.
    /// Performs remaining checks and updates the length.
    fn push_do(&mut self, subdir: String) -> Result<(), Error> {
        let depth = self.depth();
        // check for greater equal to account for the to be added subdir
        if depth >= MAX_NAMESPACE_DEPTH {
            bail!("namespace too deep, {depth} >= max {MAX_NAMESPACE_DEPTH}");
        }

        if self.len + subdir.len() + 1 > MAX_BACKUP_NAMESPACE_LENGTH {
            bail!("namespace length exceeded");
        }

        if !crate::PROXMOX_SAFE_ID_REGEX.is_match(&subdir) {
            bail!("not a valid namespace component: {subdir}");
        }

        if !self.inner.is_empty() {
            self.len += 1; // separating slash
        }
        self.len += subdir.len();
        self.inner.push(subdir);
        Ok(())
    }

    /// Return an adapter which [`fmt::Display`]s as a path with `"ns/"` prefixes in front of every
    /// component.
    pub fn display_as_path(&self) -> BackupNamespacePath {
        BackupNamespacePath(self)
    }

    /// Iterate over the subdirectories.
    pub fn components(&self) -> impl Iterator<Item = &str> + '_ {
        self.inner.iter().map(String::as_str)
    }

    /// Map NS by replacing `source_prefix` with `target_prefix`
    pub fn map_prefix(
        &self,
        source_prefix: &BackupNamespace,
        target_prefix: &BackupNamespace,
    ) -> Result<Self, Error> {
        let suffix = self
            .inner
            .strip_prefix(&source_prefix.inner[..])
            .ok_or_else(|| {
                format_err!(
                    "Failed to map namespace - {source_prefix} is not a valid prefix of {self}",
                )
            })?;

        let mut new = target_prefix.clone();
        for item in suffix {
            new.push(item.clone())?;
        }
        Ok(new)
    }

    /// Check whether adding `depth` levels of sub-namespaces exceeds the max depth limit
    pub fn check_max_depth(&self, depth: usize) -> Result<(), Error> {
        let ns_depth = self.depth();
        if ns_depth + depth > MAX_NAMESPACE_DEPTH {
            bail!(
                "namespace '{self}'s depth and recursion depth exceed limit: {ns_depth} + {depth} > {MAX_NAMESPACE_DEPTH}",
            );
        }
        Ok(())
    }

    fn acl_path_extend<'a>(&'a self, mut path: Vec<&'a str>) -> Vec<&'a str> {
        if self.is_root() {
            path
        } else {
            path.extend(self.inner.iter().map(|comp| comp.as_str()));
            path
        }
    }

    pub fn acl_path<'a>(&'a self, store: &'a str) -> Vec<&'a str> {
        self.acl_path_extend(vec!["datastore", store])
    }

    pub fn remote_acl_path<'a>(&'a self, remote: &'a str, store: &'a str) -> Vec<&'a str> {
        self.acl_path_extend(vec!["remote", remote, store])
    }

    /// Check whether this namespace contains another namespace.
    ///
    /// If so, the depth is returned.
    ///
    /// Example:
    /// ```
    /// # use pbs_api_types::BackupNamespace;
    /// let main: BackupNamespace = "a/b".parse().unwrap();
    /// let sub: BackupNamespace = "a/b/c/d".parse().unwrap();
    /// let other: BackupNamespace = "x/y".parse().unwrap();
    /// assert_eq!(main.contains(&main), Some(0));
    /// assert_eq!(main.contains(&sub), Some(2));
    /// assert_eq!(sub.contains(&main), None);
    /// assert_eq!(main.contains(&other), None);
    /// ```
    pub fn contains(&self, other: &BackupNamespace) -> Option<usize> {
        other
            .inner
            .strip_prefix(&self.inner[..])
            .map(|suffix| suffix.len())
    }
}

impl fmt::Display for BackupNamespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use std::fmt::Write;

        let mut parts = self.inner.iter();
        if let Some(first) = parts.next() {
            f.write_str(first)?;
        }
        for part in parts {
            f.write_char('/')?;
            f.write_str(part)?;
        }
        Ok(())
    }
}

serde_plain::derive_deserialize_from_fromstr!(BackupNamespace, "valid backup namespace");

impl std::str::FromStr for BackupNamespace {
    type Err = Error;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::new(name)
    }
}

serde_plain::derive_serialize_from_display!(BackupNamespace);

impl ApiType for BackupNamespace {
    const API_SCHEMA: Schema = BACKUP_NAMESPACE_SCHEMA;
}

/// Helper to format a [`BackupNamespace`] as a path component of a [`BackupGroup`].
///
/// This implements [`fmt::Display`] such that it includes the `ns/` subdirectory prefix in front of
/// every component.
pub struct BackupNamespacePath<'a>(&'a BackupNamespace);

impl fmt::Display for BackupNamespacePath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut sep = "ns/";
        for part in &self.0.inner {
            f.write_str(sep)?;
            sep = "/ns/";
            f.write_str(part)?;
        }
        Ok(())
    }
}

#[api]
/// Backup types.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupType {
    /// Virtual machines.
    Vm,

    /// Containers.
    Ct,

    /// "Host" backups.
    Host,
    // NOTE: if you add new types, don't forget to adapt the iter below!
}

impl BackupType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            BackupType::Vm => "vm",
            BackupType::Ct => "ct",
            BackupType::Host => "host",
        }
    }

    /// We used to have alphabetical ordering here when this was a string.
    const fn order(self) -> u8 {
        match self {
            BackupType::Ct => 0,
            BackupType::Host => 1,
            BackupType::Vm => 2,
        }
    }

    #[inline]
    pub fn iter() -> impl Iterator<Item = BackupType> + Send + Sync + Unpin + 'static {
        [BackupType::Vm, BackupType::Ct, BackupType::Host]
            .iter()
            .copied()
    }
}

impl fmt::Display for BackupType {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl std::str::FromStr for BackupType {
    type Err = Error;

    /// Parse a backup type.
    fn from_str(ty: &str) -> Result<Self, Error> {
        Ok(match ty {
            "ct" => BackupType::Ct,
            "host" => BackupType::Host,
            "vm" => BackupType::Vm,
            _ => bail!("invalid backup type {ty:?}"),
        })
    }
}

impl std::cmp::Ord for BackupType {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.order().cmp(&other.order())
    }
}

impl std::cmp::PartialOrd for BackupType {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[api(
    properties: {
        "backup-type": { type: BackupType },
        "backup-id": { schema: BACKUP_ID_SCHEMA },
    },
)]
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// A backup group (without a data store).
pub struct BackupGroup {
    /// Backup type.
    #[serde(rename = "backup-type")]
    pub ty: BackupType,

    /// Backup id.
    #[serde(rename = "backup-id")]
    pub id: String,
}

impl BackupGroup {
    pub fn new<T: Into<String>>(ty: BackupType, id: T) -> Self {
        Self { ty, id: id.into() }
    }

    pub fn matches(&self, filter: &crate::GroupFilter) -> bool {
        use crate::FilterType;
        match &filter.filter_type {
            FilterType::Group(backup_group) => {
                match backup_group.parse::<BackupGroup>() {
                    Ok(group) => *self == group,
                    Err(_) => false, // shouldn't happen if value is schema-checked
                }
            }
            FilterType::BackupType(ty) => self.ty == *ty,
            FilterType::Regex(regex) => regex.is_match(&self.to_string()),
        }
    }

    pub fn apply_filters(&self, filters: &[GroupFilter]) -> bool {
        // since there will only be view filter in the list, an extra iteration to get the umber of
        // include filter should not be an issue
        let is_included = if filters.iter().filter(|f| !f.is_exclude).count() == 0 {
            true
        } else {
            filters
                .iter()
                .filter(|f| !f.is_exclude)
                .any(|filter| self.matches(filter))
        };

        is_included
            && !filters
                .iter()
                .filter(|f| f.is_exclude)
                .any(|filter| self.matches(filter))
    }
}

impl AsRef<BackupGroup> for BackupGroup {
    #[inline]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl From<(BackupType, String)> for BackupGroup {
    #[inline]
    fn from(data: (BackupType, String)) -> Self {
        Self {
            ty: data.0,
            id: data.1,
        }
    }
}

impl std::cmp::Ord for BackupGroup {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let type_order = self.ty.cmp(&other.ty);
        if type_order != std::cmp::Ordering::Equal {
            return type_order;
        }

        // try to compare IDs numerically
        let id_self = self.id.parse::<u64>();
        let id_other = other.id.parse::<u64>();
        match (id_self, id_other) {
            (Ok(id_self), Ok(id_other)) => id_self.cmp(&id_other),
            (Ok(_), Err(_)) => std::cmp::Ordering::Less,
            (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
            _ => self.id.cmp(&other.id),
        }
    }
}

impl std::cmp::PartialOrd for BackupGroup {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for BackupGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.ty, self.id)
    }
}

impl std::str::FromStr for BackupGroup {
    type Err = Error;

    /// Parse a backup group.
    ///
    /// This parses strings like `vm/100".
    fn from_str(path: &str) -> Result<Self, Error> {
        let cap = GROUP_PATH_REGEX
            .captures(path)
            .ok_or_else(|| format_err!("unable to parse backup group path '{}'", path))?;

        Ok(Self {
            ty: cap.get(1).unwrap().as_str().parse()?,
            id: cap.get(2).unwrap().as_str().to_owned(),
        })
    }
}

#[api(
    properties: {
        "group": { type: BackupGroup },
        "backup-time": { schema: BACKUP_TIME_SCHEMA },
    },
)]
/// Uniquely identify a Backup (relative to data store)
///
/// We also call this a backup snaphost.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct BackupDir {
    /// Backup group.
    #[serde(flatten)]
    pub group: BackupGroup,

    /// Backup timestamp unix epoch.
    #[serde(rename = "backup-time")]
    pub time: i64,
}

impl AsRef<BackupGroup> for BackupDir {
    #[inline]
    fn as_ref(&self) -> &BackupGroup {
        &self.group
    }
}

impl AsRef<BackupDir> for BackupDir {
    #[inline]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl From<(BackupGroup, i64)> for BackupDir {
    fn from(data: (BackupGroup, i64)) -> Self {
        Self {
            group: data.0,
            time: data.1,
        }
    }
}

impl From<(BackupType, String, i64)> for BackupDir {
    fn from(data: (BackupType, String, i64)) -> Self {
        Self {
            group: (data.0, data.1).into(),
            time: data.2,
        }
    }
}

impl BackupDir {
    pub fn with_rfc3339<T>(ty: BackupType, id: T, backup_time_string: &str) -> Result<Self, Error>
    where
        T: Into<String>,
    {
        let time = proxmox_time::parse_rfc3339(backup_time_string)?;
        let group = BackupGroup::new(ty, id.into());
        Ok(Self { group, time })
    }

    #[inline]
    pub fn ty(&self) -> BackupType {
        self.group.ty
    }

    #[inline]
    pub fn id(&self) -> &str {
        &self.group.id
    }
}

impl std::str::FromStr for BackupDir {
    type Err = Error;

    /// Parse a snapshot path.
    ///
    /// This parses strings like `host/elsa/2020-06-15T05:18:33Z".
    fn from_str(path: &str) -> Result<Self, Self::Err> {
        let cap = SNAPSHOT_PATH_REGEX
            .captures(path)
            .ok_or_else(|| format_err!("unable to parse backup snapshot path '{}'", path))?;

        BackupDir::with_rfc3339(
            cap.get(1).unwrap().as_str().parse()?,
            cap.get(2).unwrap().as_str(),
            cap.get(3).unwrap().as_str(),
        )
    }
}

impl fmt::Display for BackupDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // FIXME: log error?
        let time = proxmox_time::epoch_to_rfc3339_utc(self.time).map_err(|_| fmt::Error)?;
        write!(f, "{}/{}", self.group, time)
    }
}

/// Used when both a backup group or a directory can be valid.
pub enum BackupPart {
    Group(BackupGroup),
    Dir(BackupDir),
}

impl std::str::FromStr for BackupPart {
    type Err = Error;

    /// Parse a path which can be either a backup group or a snapshot dir.
    fn from_str(path: &str) -> Result<Self, Error> {
        let cap = GROUP_OR_SNAPSHOT_PATH_REGEX
            .captures(path)
            .ok_or_else(|| format_err!("unable to parse backup snapshot path '{}'", path))?;

        let ty = cap.get(1).unwrap().as_str().parse()?;
        let id = cap.get(2).unwrap().as_str().to_string();

        Ok(match cap.get(3) {
            Some(time) => BackupPart::Dir(BackupDir::with_rfc3339(ty, id, time.as_str())?),
            None => BackupPart::Group((ty, id).into()),
        })
    }
}

#[api(
    properties: {
        "backup": { type: BackupDir },
        comment: {
            schema: SINGLE_LINE_COMMENT_SCHEMA,
            optional: true,
        },
        verification: {
            type: SnapshotVerifyState,
            optional: true,
        },
        fingerprint: {
            type: String,
            optional: true,
        },
        files: {
            items: {
                schema: BACKUP_ARCHIVE_NAME_SCHEMA
            },
        },
        owner: {
            type: Authid,
            optional: true,
        },
    },
)]
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Basic information about backup snapshot.
pub struct SnapshotListItem {
    #[serde(flatten)]
    pub backup: BackupDir,
    /// The first line from manifest "notes"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// The result of the last run verify task
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<SnapshotVerifyState>,
    /// Fingerprint of encryption key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<Fingerprint>,
    /// List of contained archive files.
    pub files: Vec<BackupContent>,
    /// Overall snapshot size (sum of all archive sizes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// The owner of the snapshots group
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<Authid>,
    /// Protection from prunes
    #[serde(default)]
    pub protected: bool,
}

#[api(
    properties: {
        "backup": { type: BackupGroup },
        "last-backup": { schema: BACKUP_TIME_SCHEMA },
        "backup-count": {
            type: Integer,
        },
        files: {
            items: {
                schema: BACKUP_ARCHIVE_NAME_SCHEMA
            },
        },
        owner: {
            type: Authid,
            optional: true,
        },
    },
)]
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Basic information about a backup group.
pub struct GroupListItem {
    #[serde(flatten)]
    pub backup: BackupGroup,

    pub last_backup: i64,
    /// Number of contained snapshots
    pub backup_count: u64,
    /// List of contained archive files.
    pub files: Vec<String>,
    /// The owner of group
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<Authid>,
    /// The first line from group "notes"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[api()]
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Basic information about a backup namespace.
pub struct NamespaceListItem {
    /// A backup namespace
    pub ns: BackupNamespace,

    // TODO?
    //pub group_count: u64,
    //pub ns_count: u64,
    /// The first line from the namespace's "notes"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[api(
    properties: {
        "backup": { type: BackupDir },
    },
)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Prune result.
pub struct PruneListItem {
    #[serde(flatten)]
    pub backup: BackupDir,

    /// Keep snapshot
    pub keep: bool,
}

#[api(
    properties: {
        ct: {
            type: TypeCounts,
            optional: true,
        },
        host: {
            type: TypeCounts,
            optional: true,
        },
        vm: {
            type: TypeCounts,
            optional: true,
        },
        other: {
            type: TypeCounts,
            optional: true,
        },
    },
)]
#[derive(Serialize, Deserialize, Default)]
/// Counts of groups/snapshots per BackupType.
pub struct Counts {
    /// The counts for CT backups
    pub ct: Option<TypeCounts>,
    /// The counts for Host backups
    pub host: Option<TypeCounts>,
    /// The counts for VM backups
    pub vm: Option<TypeCounts>,
    /// The counts for other backup types
    pub other: Option<TypeCounts>,
}

#[api()]
#[derive(Serialize, Deserialize, Default)]
/// Backup Type group/snapshot counts.
pub struct TypeCounts {
    /// The number of groups of the type.
    pub groups: u64,
    /// The number of snapshots of the type.
    pub snapshots: u64,
}

#[api()]
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
/// Garbage collection cache statistics
pub struct GarbageCollectionCacheStats {
    /// Number of atime update cache hits
    pub hits: usize,
    /// Number of atime update cache misses
    pub misses: usize,
}

#[api(
    properties: {
        "upid": {
            optional: true,
            type: UPID,
        },
    },
)]
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Garbage collection status.
pub struct GarbageCollectionStatus {
    pub upid: Option<String>,
    /// Number of processed index files.
    pub index_file_count: usize,
    /// Sum of bytes referred by index files.
    pub index_data_bytes: u64,
    /// Bytes used on disk.
    pub disk_bytes: u64,
    /// Chunks used on disk.
    pub disk_chunks: usize,
    /// Sum of removed bytes.
    pub removed_bytes: u64,
    /// Number of removed chunks.
    pub removed_chunks: usize,
    /// Sum of pending bytes (pending removal - kept for safety).
    pub pending_bytes: u64,
    /// Number of pending chunks (pending removal - kept for safety).
    pub pending_chunks: usize,
    /// Number of chunks marked as .bad by verify that have been removed by GC.
    pub removed_bad: usize,
    /// Number of chunks still marked as .bad after garbage collection.
    pub still_bad: usize,
    /// Statistics of atime update cache
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_stats: Option<GarbageCollectionCacheStats>,
}

#[api(
    properties: {
        "status": {
            type: GarbageCollectionStatus,
        },
    }
)]
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Garbage Collection general info
pub struct GarbageCollectionJobStatus {
    /// Datastore
    pub store: String,
    #[serde(flatten)]
    pub status: GarbageCollectionStatus,
    /// Schedule of the gc job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    /// Time of the next gc run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run: Option<i64>,
    /// Endtime of the last gc run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_endtime: Option<i64>,
    /// State of the last gc run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_state: Option<String>,
    /// Duration of last gc run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<i64>,
}

#[api(
    properties: {
        "gc-status": {
            type: GarbageCollectionStatus,
            optional: true,
        },
        counts: {
            type: Counts,
            optional: true,
        },
    },
)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Overall Datastore status and useful information.
pub struct DataStoreStatus {
    /// Total space (bytes).
    pub total: u64,
    /// Used space (bytes).
    pub used: u64,
    /// Available space (bytes).
    pub avail: u64,
    /// Status of last GC
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gc_status: Option<GarbageCollectionStatus>,
    /// Group/Snapshot counts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counts: Option<Counts>,
}

#[api(
    properties: {
        store: {
            schema: DATASTORE_SCHEMA,
        },
        "mount-status": {
            type: DataStoreMountStatus,
        },
        history: {
            type: Array,
            optional: true,
            items: {
                type: Number,
                description: "The usage of a time in the past. Either null or between 0.0 and 1.0.",
            }
        },
     },
)]
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Status of a Datastore
pub struct DataStoreStatusListItem {
    pub store: String,
    /// The Size of the underlying storage in bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    /// The used bytes of the underlying storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used: Option<u64>,
    /// The available bytes of the underlying storage. (-1 on error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avail: Option<u64>,
    #[serde(default)]
    pub mount_status: DataStoreMountStatus,
    /// A list of usages of the past (last Month).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<Option<f64>>>,
    /// History start time (epoch)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_start: Option<u64>,
    /// History resolution (seconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_delta: Option<u64>,
    /// Estimation of the UNIX epoch when the storage will be full.
    /// It's calculated via a simple Linear Regression (Least Squares) over the RRD data of the
    /// last Month. Missing if not enough data points are available yet. An estimate in the past
    /// means that usage is declining or not changing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_full_date: Option<i64>,
    /// An error description, for example, when the datastore could not be looked up
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Status of last GC
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gc_status: Option<GarbageCollectionStatus>,
}

impl DataStoreStatusListItem {
    pub fn empty(store: &str, err: Option<String>, mount_status: DataStoreMountStatus) -> Self {
        DataStoreStatusListItem {
            store: store.to_owned(),
            total: None,
            used: None,
            avail: None,
            mount_status,
            history: None,
            history_start: None,
            history_delta: None,
            estimated_full_date: None,
            error: err,
            gc_status: None,
        }
    }
}

pub const ADMIN_DATASTORE_LIST_SNAPSHOTS_RETURN_TYPE: ReturnType = ReturnType {
    optional: false,
    schema: &ArraySchema::new(
        "Returns the list of snapshots.",
        &SnapshotListItem::API_SCHEMA,
    )
    .schema(),
};

pub const ADMIN_DATASTORE_LIST_SNAPSHOT_FILES_RETURN_TYPE: ReturnType = ReturnType {
    optional: false,
    schema: &ArraySchema::new(
        "Returns the list of archive files inside a backup snapshots.",
        &BackupContent::API_SCHEMA,
    )
    .schema(),
};

pub const ADMIN_DATASTORE_LIST_GROUPS_RETURN_TYPE: ReturnType = ReturnType {
    optional: false,
    schema: &ArraySchema::new(
        "Returns the list of backup groups.",
        &GroupListItem::API_SCHEMA,
    )
    .schema(),
};

pub const ADMIN_DATASTORE_LIST_NAMESPACE_RETURN_TYPE: ReturnType = ReturnType {
    optional: false,
    schema: &ArraySchema::new(
        "Returns the list of backup namespaces.",
        &NamespaceListItem::API_SCHEMA,
    )
    .schema(),
};

pub const ADMIN_DATASTORE_PRUNE_RETURN_TYPE: ReturnType = ReturnType {
    optional: false,
    schema: &ArraySchema::new(
        "Returns the list of snapshots and a flag indicating if there are kept or removed.",
        &PruneListItem::API_SCHEMA,
    )
    .schema(),
};

#[api(
    properties: {
        store: {
            schema: DATASTORE_SCHEMA,
        },
        "max-depth": {
            schema: NS_MAX_DEPTH_SCHEMA,
            optional: true,
        },
     },
)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// A namespace mapping
pub struct TapeRestoreNamespace {
    /// The source datastore
    pub store: String,
    /// The source namespace. Root namespace if omitted.
    pub source: Option<BackupNamespace>,
    /// The target namespace,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<BackupNamespace>,
    /// The (optional) recursion depth
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<usize>,
}

pub const TAPE_RESTORE_NAMESPACE_SCHEMA: Schema = StringSchema::new("A namespace mapping")
    .format(&ApiStringFormat::PropertyString(
        &TapeRestoreNamespace::API_SCHEMA,
    ))
    .schema();

/// Parse snapshots in the form 'ns/foo/ns/bar/ct/100/1970-01-01T00:00:00Z'
/// into a [`BackupNamespace`] and [`BackupDir`]
pub fn parse_ns_and_snapshot(input: &str) -> Result<(BackupNamespace, BackupDir), Error> {
    match input.rmatch_indices('/').nth(2) {
        Some((idx, _)) => {
            let ns = BackupNamespace::from_path(&input[..idx])?;
            let dir: BackupDir = input[(idx + 1)..].parse()?;
            Ok((ns, dir))
        }
        None => Ok((BackupNamespace::root(), input.parse()?)),
    }
}

/// Prints a [`BackupNamespace`] and [`BackupDir`] in the form of
/// 'ns/foo/bar/ct/100/1970-01-01T00:00:00Z'
pub fn print_ns_and_snapshot(ns: &BackupNamespace, dir: &BackupDir) -> String {
    if ns.is_root() {
        dir.to_string()
    } else {
        format!("{}/{}", ns.display_as_path(), dir)
    }
}

/// Prints a Datastore name and [`BackupNamespace`] for logs/errors.
pub fn print_store_and_ns(store: &str, ns: &BackupNamespace) -> String {
    if ns.is_root() {
        format!("datastore '{}', root namespace", store)
    } else {
        format!("datastore '{}', namespace '{}'", store, ns)
    }
}

pub const DELETE_STATS_COUNT_SCHEMA: Schema =
    IntegerSchema::new("Number of entities").minimum(0).schema();

#[api(
    properties: {
        "removed-groups": {
            schema: DELETE_STATS_COUNT_SCHEMA,
        },
        "protected-snapshots": {
            schema: DELETE_STATS_COUNT_SCHEMA,
        },
        "removed-snapshots": {
            schema: DELETE_STATS_COUNT_SCHEMA,
        },
     },
)]
#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Statistics for removed backup groups
pub struct BackupGroupDeleteStats {
    // Count of removed groups
    removed_groups: usize,
    // Count of protected snapshots, therefore not removed
    protected_snapshots: usize,
    // Count of deleted snapshots
    removed_snapshots: usize,
}

impl BackupGroupDeleteStats {
    pub fn all_removed(&self) -> bool {
        self.protected_snapshots == 0
    }

    pub fn removed_groups(&self) -> usize {
        self.removed_groups
    }

    pub fn removed_snapshots(&self) -> usize {
        self.removed_snapshots
    }

    pub fn protected_snapshots(&self) -> usize {
        self.protected_snapshots
    }

    pub fn add(&mut self, rhs: &Self) {
        self.removed_groups += rhs.removed_groups;
        self.protected_snapshots += rhs.protected_snapshots;
        self.removed_snapshots += rhs.removed_snapshots;
    }

    pub fn increment_removed_groups(&mut self) {
        self.removed_groups += 1;
    }

    pub fn increment_removed_snapshots(&mut self) {
        self.removed_snapshots += 1;
    }

    pub fn increment_protected_snapshots(&mut self) {
        self.protected_snapshots += 1;
    }
}

#[derive(Clone, PartialEq, Eq)]
/// Allowed variants of backup archives to be contained in a snapshot's manifest
pub enum ArchiveType {
    FixedIndex,
    DynamicIndex,
    Blob,
}

impl ArchiveType {
    pub fn from_path(archive_name: impl AsRef<Path>) -> Result<Self, Error> {
        let archive_name = archive_name.as_ref();
        let archive_type = match archive_name.extension().and_then(|ext| ext.to_str()) {
            Some("didx") => ArchiveType::DynamicIndex,
            Some("fidx") => ArchiveType::FixedIndex,
            Some("blob") => ArchiveType::Blob,
            _ => bail!("unknown archive type: {archive_name:?}"),
        };
        Ok(archive_type)
    }

    pub fn extension(&self) -> &'static str {
        match self {
            ArchiveType::DynamicIndex => "didx",
            ArchiveType::FixedIndex => "fidx",
            ArchiveType::Blob => "blob",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
/// Name of archive files contained in snapshot's manifest
pub struct BackupArchiveName {
    // archive name including the `.fidx`, `.didx` or `.blob` archive type extension
    name: String,
    // archive type parsed based on given extension
    ty: ArchiveType,
}

pub static MANIFEST_BLOB_NAME: LazyLock<BackupArchiveName> = LazyLock::new(|| BackupArchiveName {
    name: "index.json.blob".to_string(),
    ty: ArchiveType::Blob,
});

pub static CATALOG_NAME: LazyLock<BackupArchiveName> = LazyLock::new(|| BackupArchiveName {
    name: "catalog.pcat1.didx".to_string(),
    ty: ArchiveType::DynamicIndex,
});

pub static CLIENT_LOG_BLOB_NAME: LazyLock<BackupArchiveName> =
    LazyLock::new(|| BackupArchiveName {
        name: "client.log.blob".to_string(),
        ty: ArchiveType::Blob,
    });

pub static ENCRYPTED_KEY_BLOB_NAME: LazyLock<BackupArchiveName> =
    LazyLock::new(|| BackupArchiveName {
        name: "rsa-encrypted.key.blob".to_string(),
        ty: ArchiveType::Blob,
    });

impl fmt::Display for BackupArchiveName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{name}", name = self.name)
    }
}

serde_plain::derive_deserialize_from_fromstr!(BackupArchiveName, "archive name");

impl FromStr for BackupArchiveName {
    type Err = Error;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::try_from(name)
    }
}

serde_plain::derive_serialize_from_display!(BackupArchiveName);

impl TryFrom<&str> for BackupArchiveName {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (name, ty) = Self::parse_archive_type(value)?;
        Ok(Self { name, ty })
    }
}

impl AsRef<str> for BackupArchiveName {
    fn as_ref(&self) -> &str {
        &self.name
    }
}

impl BackupArchiveName {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        if path.as_os_str().as_encoded_bytes().last() == Some(&b'/') {
            bail!("invalid archive name, got directory");
        }
        let file_name = path
            .file_name()
            .ok_or_else(|| format_err!("invalid archive name"))?;
        let name = file_name
            .to_str()
            .ok_or_else(|| format_err!("archive name not valid UTF-8"))?;

        Self::try_from(name)
    }

    pub fn archive_type(&self) -> ArchiveType {
        self.ty.clone()
    }

    pub fn ends_with(&self, postfix: &str) -> bool {
        self.name.ends_with(postfix)
    }

    pub fn has_pxar_filename_extension(&self) -> bool {
        self.name.ends_with(".pxar.didx")
            || self.name.ends_with(".mpxar.didx")
            || self.name.ends_with(".ppxar.didx")
    }

    pub fn without_type_extension(&self) -> String {
        self.name
            .strip_suffix(&format!(".{ext}", ext = self.ty.extension()))
            .unwrap()
            .into()
    }

    fn parse_archive_type(archive_name: &str) -> Result<(String, ArchiveType), Error> {
        // Detect archive type via given server archive name type extension, if present
        if let Ok(archive_type) = ArchiveType::from_path(archive_name) {
            return Ok((archive_name.into(), archive_type));
        }

        // No server archive name type extension in archive name, map based on extension
        let archive_type = match Path::new(archive_name)
            .extension()
            .and_then(|ext| ext.to_str())
        {
            Some("pxar") => ArchiveType::DynamicIndex,
            Some("mpxar") => ArchiveType::DynamicIndex,
            Some("ppxar") => ArchiveType::DynamicIndex,
            Some("pcat1") => ArchiveType::DynamicIndex,
            Some("img") => ArchiveType::FixedIndex,
            Some("conf") => ArchiveType::Blob,
            Some("json") => ArchiveType::Blob,
            Some("key") => ArchiveType::Blob,
            Some("log") => ArchiveType::Blob,
            _ => bail!("failed to parse archive type for '{archive_name}'"),
        };

        Ok((
            format!("{archive_name}.{ext}", ext = archive_type.extension()),
            archive_type,
        ))
    }
}

impl ApiType for BackupArchiveName {
    const API_SCHEMA: Schema = BACKUP_ARCHIVE_NAME_SCHEMA;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_backup_archive_names() {
        let invalid_archive_names = [
            "/invalid/",
            "/invalid/archive-name.pxar/",
            "/invalid/archive-name.pxar.didx/",
            "/invalid/..",
            "/invalid/archive-name.invalid",
        ];

        for archive_name in invalid_archive_names {
            assert!(BackupArchiveName::from_path(archive_name).is_err());
        }
    }

    #[test]
    fn test_valid_didx_backup_archive_names() {
        let valid_archive_names = [
            "/valid/archive-name.pxar",
            "/valid/archive-name.pxar.didx",
            "/valid/archive-name.mpxar",
            "/valid/archive-name.mpxar.didx",
            "/valid/archive-name.ppxar",
            "/valid/archive-name.ppxar.didx",
            "/valid/archive-name.pcat1",
            "/valid/archive-name.pcat1.didx",
        ];

        for archive_name in valid_archive_names {
            let archive = BackupArchiveName::from_path(archive_name).unwrap();
            assert!(archive.as_ref().ends_with(".didx"));
            assert!(archive.archive_type() == ArchiveType::DynamicIndex);
        }
    }

    #[test]
    fn test_valid_fidx_backup_archive_names() {
        let valid_archive_names = ["/valid/archive-name.img", "/valid/archive-name.img.fidx"];

        for archive_name in valid_archive_names {
            let archive = BackupArchiveName::from_path(archive_name).unwrap();
            assert!(archive.as_ref() == "archive-name.img.fidx");
            assert!(archive.without_type_extension() == "archive-name.img");
            assert!(archive.archive_type() == ArchiveType::FixedIndex);
        }
    }

    #[test]
    fn test_valid_blob_backup_archive_names() {
        let valid_archive_names = [
            "/valid/index.json",
            "/valid/index.json.blob",
            "/valid/rsa-encrypted.key",
            "/valid/rsa-encrypted.key.blob",
            "/valid/archive-name.log",
            "/valid/archive-name.log.blob",
            "/valid/qemu-server.conf",
            "/valid/qemu-server.conf.blob",
        ];

        for archive_name in valid_archive_names {
            let archive = BackupArchiveName::from_path(archive_name).unwrap();
            assert!(archive.as_ref().ends_with(".blob"));
            assert!(archive.archive_type() == ArchiveType::Blob);
        }
    }
}
