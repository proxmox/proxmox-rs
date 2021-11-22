//! File system related utilities 
use std::fs::File;
use std::path::Path;

use anyhow::{bail, Error};

use std::os::unix::io::{AsRawFd, RawFd};
use nix::unistd::{Gid, Uid};
use nix::sys::stat;
use nix::errno::Errno;

mod file;
pub use file::*;

mod dir;
pub use dir::*;

mod read_dir;
pub use read_dir::*;

mod fsx_attr;
pub use fsx_attr::*;

/// Change ownership of an open file handle
pub fn fchown(fd: RawFd, owner: Option<Uid>, group: Option<Gid>) -> Result<(), Error> {
    // According to the POSIX specification, -1 is used to indicate that owner and group
    // are not to be changed.  Since uid_t and gid_t are unsigned types, we have to wrap
    // around to get -1 (copied fron nix crate).
    let uid = owner.map(Into::into).unwrap_or(!(0 as libc::uid_t));
    let gid = group.map(Into::into).unwrap_or(!(0 as libc::gid_t));

    let res = unsafe { libc::fchown(fd, uid, gid) };
    Errno::result(res)?;

    Ok(())
}

// FIXME: Consider using derive-builder!
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

    pub fn apply_to(&self, file: &mut File, path: &Path) -> Result<(), Error> {

        // clippy bug?: from_bits_truncate is actually a const fn...
        #[allow(clippy::or_fun_call)]
        let mode: stat::Mode = self.perm
            .unwrap_or(stat::Mode::from_bits_truncate(0o644));

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

    // TODO: once 'nix' has `const fn` constructors for Uid and Gid we can enable these:

    /*
    /// Convenience shortcut around having to import `Gid` from nix.
    pub const fn group_root(self) -> Self {
        // nix hasn't constified these yet, but it's just an alias to gid_t:
        self.group(Gid::from_raw(0))
    }

    /// Convenience shortcut to set both owner and group to 0.
    pub const fn root_only(self) -> Self {
        self.owner_root().group_root()
    }
    */
}

