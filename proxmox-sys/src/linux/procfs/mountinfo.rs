//! `/proc/PID/mountinfo` handling.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{bail, format_err, Error};
use nix::sys::stat;
use nix::unistd::Pid;

/// A mount ID as found within `/proc/PID/mountinfo`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct MountId(usize);

impl FromStr for MountId {
    type Err = <usize as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self)
    }
}

/// A device node entry (major:minor). This is a more strongly typed version of dev_t.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Device {
    major: u32,
    minor: u32,
}

impl Device {
    pub fn from_dev_t(dev: stat::dev_t) -> Self {
        Self {
            major: stat::major(dev) as u32,
            minor: stat::minor(dev) as u32,
        }
    }

    pub fn into_dev_t(self) -> stat::dev_t {
        stat::makedev(u64::from(self.major), u64::from(self.minor))
    }
}

impl FromStr for Device {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        let (major, minor) = s.split_at(
            s.find(':')
                .ok_or_else(|| format_err!("expected 'major:minor' format"))?,
        );
        Ok(Self {
            major: major.parse()?,
            minor: minor[1..].parse()?,
        })
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct Tag {
    pub tag: OsString,
    pub value: Option<OsString>,
}

impl Tag {
    fn parse(tag: &[u8]) -> Result<Self, Error> {
        Ok(match tag.iter().position(|b| *b == b':') {
            Some(pos) => {
                let (tag, value) = tag.split_at(pos);
                Self {
                    tag: OsStr::from_bytes(tag).to_owned(),
                    value: Some(OsStr::from_bytes(&value[1..]).to_owned()),
                }
            }
            None => Self {
                tag: OsStr::from_bytes(tag).to_owned(),
                value: None,
            },
        })
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    /// unique identifier of the mount (may be reused after being unmounted)
    pub id: MountId,

    /// id of the parent (or of self for the top of the mount tree)
    pub parent: MountId,

    /// value of st_dev for files on this file system
    pub device: Device,

    /// root of the mount within the file system
    pub root: PathBuf,

    /// mount point relative to the process' root
    pub mount_point: PathBuf,

    /// per-mount options
    pub mount_options: OsString,

    /// tags
    pub tags: Vec<Tag>,

    /// Name of the file system in the form "type[.subtype]".
    pub fs_type: String,

    /// File system specific mount source information.
    pub mount_source: Option<OsString>,

    /// superblock options
    pub super_options: OsString,
}

impl Entry {
    /// Parse a line from a `mountinfo` file.
    pub fn parse(line: &[u8]) -> Result<Self, Error> {
        let mut parts = line.split(u8::is_ascii_whitespace);

        let mut next = || {
            parts
                .next()
                .ok_or_else(|| format_err!("incomplete mountinfo line"))
        };

        let this = Self {
            id: std::str::from_utf8(next()?)?.parse()?,
            parent: std::str::from_utf8(next()?)?.parse()?,
            device: std::str::from_utf8(next()?)?.parse()?,
            root: OsStr::from_bytes(next()?).to_owned().into(),
            mount_point: OsStr::from_bytes(next()?).to_owned().into(),
            mount_options: OsStr::from_bytes(next()?).to_owned(),
            tags: {
                let mut tags = Vec::new();
                loop {
                    let tval = next()?;
                    if tval == b"-" {
                        break;
                    }
                    tags.push(Tag::parse(tval)?);
                }
                tags
            },
            fs_type: std::str::from_utf8(next()?)?.to_string(),
            mount_source: next().map(|src| match src {
                b"none" => None,
                other => Some(OsStr::from_bytes(other).to_owned()),
            })?,
            super_options: OsStr::from_bytes(next()?).to_owned(),
        };

        if parts.next().is_some() {
            bail!("excess data in mountinfo line");
        }

        Ok(this)
    }
}

// TODO: Add some structure to this? Eg. sort by parent/child relation? Make a tree?
/// Mount info found in `/proc/PID/mountinfo`.
#[derive(Clone, Debug)]
pub struct MountInfo {
    entries: BTreeMap<MountId, Entry>,
}

/// An iterator over entries in a `MountInfo`.
pub type Iter<'a> = std::collections::btree_map::Iter<'a, MountId, Entry>;

/// An iterator over mutable entries in a `MountInfo`.
pub type IterMut<'a> = std::collections::btree_map::IterMut<'a, MountId, Entry>;

impl MountInfo {
    /// Read the current mount point information.
    pub fn read() -> Result<Self, Error> {
        Self::parse(&std::fs::read("/proc/self/mountinfo")?)
    }

    /// Read the mount point information of a specific pid.
    pub fn read_for_pid(pid: Pid) -> Result<Self, Error> {
        Self::parse(&std::fs::read(format!("/proc/{pid}/mountinfo"))?)
    }

    /// Parse a `mountinfo` file.
    pub fn parse(statstr: &[u8]) -> Result<Self, Error> {
        let entries = statstr
            .split(|b| *b == b'\n')
            .filter(|line| !line.is_empty())
            .try_fold(Vec::new(), |mut acc, line| -> Result<_, Error> {
                let entry = match Entry::parse(line) {
                    Ok(entry) => entry,
                    Err(err) => {
                        bail!(
                            "failed to parse mount info line: {:?}\n    error: {}",
                            line,
                            err,
                        );
                    }
                };
                acc.push(entry);
                Ok(acc)
            })?;

        let entries = entries.into_iter().map(|entry| (entry.id, entry)).collect();

        Ok(Self { entries })
    }

    /// Iterate over mount entries.
    pub fn iter(&self) -> Iter<'_> {
        self.entries.iter()
    }

    /// Check if there exists a mount point for a specific path.
    ///
    /// FIXME: Do we need to verify that mount points don't get "hidden" by other higher level
    /// mount points? For this we'd need to implement mountpoint-tree iteration first, the info for
    /// which we have available in the `Entry` struct!
    pub fn path_is_mounted<P>(&self, path: &P) -> bool
    where
        PathBuf: PartialEq<P>,
    {
        self.iter().any(|(_id, entry)| entry.mount_point == *path)
    }

    /// Check whether there exists a mount point for a specified source.
    pub fn source_is_mounted<T>(&self, source: &T) -> bool
    where
        OsString: PartialEq<T>,
    {
        self.iter()
            .filter_map(|(_id, entry)| entry.mount_source.as_ref())
            .any(|s| *s == *source)
    }
}

impl IntoIterator for MountInfo {
    type Item = (MountId, Entry);
    type IntoIter = std::collections::btree_map::IntoIter<MountId, Entry>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<'a> IntoIterator for &'a MountInfo {
    type Item = (&'a MountId, &'a Entry);
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl<'a> IntoIterator for &'a mut MountInfo {
    type Item = (&'a MountId, &'a mut Entry);
    type IntoIter = IterMut<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter_mut()
    }
}

impl std::ops::Deref for MountInfo {
    type Target = BTreeMap<MountId, Entry>;

    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

impl std::ops::DerefMut for MountInfo {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.entries
    }
}

#[test]
fn test_entry() {
    use std::path::Path;

    let l1: &[u8] =
        b"48 32 0:43 / /sys/fs/cgroup/blkio rw,nosuid,nodev,noexec,relatime shared:26 - cgroup \
          cgroup rw,blkio";
    let entry = Entry::parse(l1).expect("failed to parse first mountinfo test entry");

    assert_eq!(entry.id, MountId(48));
    assert_eq!(entry.parent, MountId(32));
    assert_eq!(
        entry.device,
        Device {
            major: 0,
            minor: 43,
        }
    );
    assert_eq!(entry.root, Path::new("/"));
    assert_eq!(entry.mount_point, Path::new("/sys/fs/cgroup/blkio"));
    assert_eq!(
        entry.mount_options,
        OsStr::new("rw,nosuid,nodev,noexec,relatime")
    );
    assert_eq!(
        entry.tags,
        &[Tag {
            tag: OsString::from("shared"),
            value: Some(OsString::from("26")),
        }]
    );
    assert_eq!(entry.fs_type, "cgroup");
    assert_eq!(entry.mount_source.as_deref(), Some(OsStr::new("cgroup")));
    assert_eq!(entry.super_options, "rw,blkio");

    let l2 = b"49 28 0:44 / /proxmox/debian rw,relatime shared:27 - autofs systemd-1 \
               rw,fd=26,pgrp=1,timeout=0,minproto=5,maxproto=5,direct,pipe_ino=27726";
    let entry = Entry::parse(l2).expect("failed to parse second mountinfo test entry");
    assert_eq!(entry.id, MountId(49));
    assert_eq!(entry.parent, MountId(28));
    assert_eq!(
        entry.device,
        Device {
            major: 0,
            minor: 44,
        }
    );
    assert_eq!(entry.root, Path::new("/"));
    assert_eq!(entry.mount_point, Path::new("/proxmox/debian"));
    assert_eq!(entry.mount_options, OsStr::new("rw,relatime"));
    assert_eq!(
        entry.tags,
        &[Tag {
            tag: OsString::from("shared"),
            value: Some(OsString::from("27")),
        }]
    );
    assert_eq!(entry.fs_type, "autofs");
    assert_eq!(entry.mount_source.as_deref(), Some(OsStr::new("systemd-1")));
    assert_eq!(
        entry.super_options,
        "rw,fd=26,pgrp=1,timeout=0,minproto=5,maxproto=5,direct,pipe_ino=27726"
    );

    // test different tag configurations
    let l3: &[u8] = b"225 224 0:46 / /proc rw,nosuid,nodev,noexec,relatime - proc proc rw";
    let entry = Entry::parse(l3).expect("failed to parse third mountinfo test entry");
    assert_eq!(entry.tags, &[]);

    let l4: &[u8] = b"48 32 0:43 / /sys/fs/cgroup/blkio rw,nosuid,nodev,noexec,relatime \
          shared:5 master:7 propagate_from:2 unbindable \
          - cgroup cgroup rw,blkio";
    let entry = Entry::parse(l4).expect("failed to parse fourth mountinfo test entry");
    assert_eq!(
        entry.tags,
        &[
            Tag {
                tag: OsString::from("shared"),
                value: Some(OsString::from("5")),
            },
            Tag {
                tag: OsString::from("master"),
                value: Some(OsString::from("7")),
            },
            Tag {
                tag: OsString::from("propagate_from"),
                value: Some(OsString::from("2")),
            },
            Tag {
                tag: OsString::from("unbindable"),
                value: None,
            },
        ]
    );

    let mount_info = [l1, l2].join(&b"\n"[..]);
    MountInfo::parse(&mount_info).expect("failed to parse mount info file");
}
