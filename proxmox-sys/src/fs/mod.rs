//! File system related utilities
use std::path::Path;

use anyhow::{bail, Context, Error};

use nix::sys::stat;
use nix::unistd::{Gid, Uid};
use std::os::unix::io::{AsRawFd, RawFd};

#[cfg(feature = "acl")]
pub mod acl;

mod file;
pub use file::*;

mod dir;
pub use dir::*;

mod read_dir;
pub use read_dir::*;

mod fsx_attr;
pub use fsx_attr::*;

pub mod xattr;

/// Change ownership of an open file handle
pub fn fchown(fd: RawFd, owner: Option<Uid>, group: Option<Gid>) -> Result<(), Error> {
    nix::unistd::fchown(fd, owner, group).map_err(|err| err.into())
}

/// Define permissions, owner and group when creating files/dirs
#[derive(Clone, Default)]
pub struct CreateOptions {
    perm: Option<stat::Mode>,
    owner: Option<Uid>,
    group: Option<Gid>,
}

impl CreateOptions {
    // contrary to Default::default() this is const
    pub const fn new() -> Self {
        Self {
            perm: None,
            owner: None,
            group: None,
        }
    }

    pub const fn perm(mut self, perm: stat::Mode) -> Self {
        self.perm = Some(perm);
        self
    }

    pub const fn owner(mut self, owner: Uid) -> Self {
        self.owner = Some(owner);
        self
    }

    pub const fn group(mut self, group: Gid) -> Self {
        self.group = Some(group);
        self
    }

    /// Convenience shortcut around having to import `Uid` from nix.
    pub const fn owner_root(self) -> Self {
        self.owner(nix::unistd::ROOT)
    }

    pub fn apply_to<F: AsRawFd>(&self, file: &mut F, path: &Path) -> Result<(), Error> {
        // clippy bug?: from_bits_truncate is actually a const fn...
        #[allow(clippy::or_fun_call)]
        let mode: stat::Mode = self.perm.unwrap_or(stat::Mode::from_bits_truncate(0o644));

        if let Err(err) = stat::fchmod(file.as_raw_fd(), mode) {
            bail!("fchmod {:?} failed: {}", path, err);
        }

        if self.owner.is_some() || self.group.is_some() {
            if let Err(err) = fchown(file.as_raw_fd(), self.owner, self.group) {
                bail!("fchown {:?} failed: {}", path, err);
            }
        }
        Ok(())
    }

    /// Check file/directory permissions.
    ///
    /// Make sure that the file or dir is owned by uid/gid and has the correct mode.
    pub fn check<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        let path = path.as_ref();

        let nix::sys::stat::FileStat {
            st_uid,
            st_gid,
            st_mode,
            ..
        } = nix::sys::stat::stat(path).with_context(|| format!("failed to stat {path:?}"))?;

        if let Some(uid) = self.owner {
            let uid = uid.as_raw();
            if st_uid != uid {
                bail!("bad owner on {path:?} ({st_uid} != {uid})");
            }
        }

        if let Some(group) = self.group {
            let gid = group.as_raw();
            if st_gid != gid {
                bail!("bad group on {path:?} ({st_gid} != {gid})");
            }
        }

        if let Some(mode) = self.perm {
            let mode = mode.bits();
            let perms = st_mode & !nix::sys::stat::SFlag::S_IFMT.bits();
            if perms != mode {
                bail!("bad permissions on {path:?} (0o{perms:o} != 0o{mode:o})");
            }
        }

        Ok(())
    }

    /// Convenience shortcut around having to import `Gid` from nix.
    pub const fn group_root(self) -> Self {
        self.group(Gid::from_raw(0))
    }

    /// Convenience shortcut to set both owner and group to 0.
    pub const fn root_only(self) -> Self {
        self.owner_root().group_root()
    }
}

/// Information about a mounted file system from statfs64 syscall
pub struct FileSystemInformation {
    /// total bytes available
    pub total: u64,
    /// bytes used
    pub used: u64,
    /// bytes available to an unprivileged user
    pub available: u64,
    /// total number of inodes
    pub total_inodes: u64,
    /// free number of inodes
    pub free_inodes: u64,
    /// the type of the filesystem (see statfs64(2))
    pub fs_type: i64,
    /// the filesystem id
    pub fs_id: libc::fsid_t,
}

/// Get file system information from path
pub fn fs_info<P: ?Sized + nix::NixPath>(path: &P) -> nix::Result<FileSystemInformation> {
    let mut stat: libc::statfs64 = unsafe { std::mem::zeroed() };

    let res = path.with_nix_path(|cstr| unsafe { libc::statfs64(cstr.as_ptr(), &mut stat) })?;
    nix::errno::Errno::result(res)?;

    let block_size = if stat.f_frsize == 0 {
        stat.f_bsize as u64
    } else {
        stat.f_frsize as u64
    };

    Ok(FileSystemInformation {
        total: stat.f_blocks * block_size,
        used: (stat.f_blocks - stat.f_bfree) * block_size,
        available: stat.f_bavail * block_size,
        total_inodes: stat.f_files,
        free_inodes: stat.f_ffree,
        fs_type: stat.f_type,
        fs_id: stat.f_fsid,
    })
}
