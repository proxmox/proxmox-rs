//! PVE LXC related schema module.

use proxmox::api::api;

#[api({
    description: "A long-term lock on a container",
})]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigLock {
    Backup,
    Create,
    Disk,
    Fstrim,
    Migrate,
    Mounted,
    Rollback,
    Snapshot,
    #[api(rename = "snapshot-delete")]
    SnapshotDelete,
}

#[api({
    description: "Operating System Type.",
})]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OsType {
    Unmanaged,
    Debian,
    //...
}

#[api({
    description: "Console mode.",
})]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsoleMode {
    Tty,
    Console,
    Shell,
}

pub mod mount_options {
    pub const NAME: &'static str = "mount options";

    const VALID_MOUNT_OPTIONS: &[&'static str] = &["noatime", "nodev", "noexec", "nosuid"];

    pub fn verify<T: crate::schema::tools::StringContainer>(value: &T) -> bool {
        value.all(|s| VALID_MOUNT_OPTIONS.contains(&s))
    }
}
