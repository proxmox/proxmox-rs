//! File related utilities such as `replace_file`.

use std::ffi::CStr;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::path::Path;

use anyhow::{bail, format_err, Error};
use nix::errno::Errno;
use nix::fcntl::OFlag;
use nix::sys::stat;
use nix::unistd::{self, Gid, Uid};
use serde_json::Value;

use crate::tools::fd::Fd;
use crate::try_block;

/// Read the entire contents of a file into a bytes vector
///
/// This basically call ``std::fs::read``, but provides more elaborate
/// error messages including the path.
pub fn file_get_contents<P: AsRef<Path>>(path: P) -> Result<Vec<u8>, Error> {
    let path = path.as_ref();

    std::fs::read(path).map_err(|err| format_err!("unable to read {:?} - {}", path, err))
}

/// Read .json file into a ``Value``
///
/// The optional ``default`` is used when the file does not exist.
pub fn file_get_json<P: AsRef<Path>>(path: P, default: Option<Value>) -> Result<Value, Error> {
    let path = path.as_ref();

    let raw = match std::fs::read(path) {
        Ok(v) => v,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                if let Some(v) = default {
                    return Ok(v);
                }
            }
            bail!("unable to read json {:?} - {}", path, err);
        }
    };

    try_block!({
        let data = String::from_utf8(raw)?;
        let json = serde_json::from_str(&data)?;
        Ok(json)
    })
    .map_err(|err: Error| format_err!("unable to parse json from {:?} - {}", path, err))
}

/// Read the first line of a file as String
pub fn file_read_firstline<P: AsRef<Path>>(path: P) -> Result<String, Error> {
    let path = path.as_ref();

    try_block!({
        let file = std::fs::File::open(path)?;

        let mut reader = BufReader::new(file);

        let mut line = String::new();

        let _ = reader.read_line(&mut line)?;

        Ok(line)
    })
    .map_err(|err: Error| format_err!("unable to read {:?} - {}", path, err))
}

/// Atomically replace a file.
///
/// This first creates a temporary file and then rotates it in place.
pub fn replace_file<P: AsRef<Path>>(
    path: P,
    data: &[u8],
    options: CreateOptions,
) -> Result<(), Error> {
    let path = path.as_ref();

    // Note: we use mkstemp heŕe, because this worka with different
    // processes, threads, and even tokio tasks.
    let mut template = path.to_owned();
    template.set_extension("tmp_XXXXXX");
    let (fd, tmp_path) = match unistd::mkstemp(&template) {
        Ok((fd, path)) => (fd, path),
        Err(err) => bail!("mkstemp {:?} failed: {}", template, err),
    };

    let tmp_path = tmp_path.as_path();

    // clippy bug?: from_bits_truncate is actually a const fn...
    #[allow(clippy::or_fun_call)]
    let mode: stat::Mode = options
        .perm
        .unwrap_or(stat::Mode::from_bits_truncate(0o644));

    if let Err(err) = stat::fchmod(fd, mode) {
        let _ = unistd::unlink(tmp_path);
        bail!("fchmod {:?} failed: {}", tmp_path, err);
    }

    if options.owner.is_some() || options.group.is_some() {
        if let Err(err) = fchown(fd, options.owner, options.group) {
            let _ = unistd::unlink(tmp_path);
            bail!("fchown {:?} failed: {}", tmp_path, err);
        }
    }

    let mut file = unsafe { File::from_raw_fd(fd) };

    if let Err(err) = file.write_all(data) {
        let _ = unistd::unlink(tmp_path);
        bail!("write failed: {}", err);
    }

    if let Err(err) = std::fs::rename(tmp_path, path) {
        let _ = unistd::unlink(tmp_path);
        bail!("Atomic rename failed for file {:?} - {}", path, err);
    }

    Ok(())
}

/// Atomically replace a file.
///
/// This first creates a temporary file and then rotates it in place.
///
/// Deprecated for the following reasons:
///   1) The name suggests that the contents of an existing file is changed.
///   2) The function is split into this and a `_full` counterpart of which you need to remember
///      the order of parameters. But we already introduced `CreateOptions` which can be used to
///      provide a better API for both.
///   3) `CreateOptions` is a builder, so more options can be added without having to change all
///      its users!
#[deprecated(note = "use replace_file instead")]
pub fn file_set_contents<P: AsRef<Path>>(
    path: P,
    data: &[u8],
    perm: Option<stat::Mode>,
) -> Result<(), Error> {
    #[allow(deprecated)]
    file_set_contents_full(path.as_ref(), data, perm, None, None)
}

/// Atomically write a file with owner and group
#[deprecated(note = "use replace_file instead")]
pub fn file_set_contents_full<P: AsRef<Path>>(
    path: P,
    data: &[u8],
    perm: Option<stat::Mode>,
    owner: Option<Uid>,
    group: Option<Gid>,
) -> Result<(), Error> {
    let mut options = CreateOptions::new();
    if let Some(perm) = perm {
        options = options.perm(perm);
    }
    if let Some(owner) = owner {
        options = options.owner(owner);
    }
    if let Some(group) = group {
        options = options.group(group);
    }
    replace_file(path.as_ref(), data, options)
}

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

    pub fn perm(mut self, perm: stat::Mode) -> Self {
        self.perm = Some(perm);
        self
    }

    pub fn owner(mut self, owner: Uid) -> Self {
        self.owner = Some(owner);
        self
    }

    pub fn group(mut self, group: Gid) -> Self {
        self.group = Some(group);
        self
    }
}

/// Creates directory at the provided path with specified ownership.
///
/// Errors if the directory already exists.
pub fn create_dir<P: AsRef<Path>>(path: P, options: CreateOptions) -> Result<(), nix::Error> {
    // clippy bug?: from_bits_truncate is actually a const fn...
    #[allow(clippy::or_fun_call)]
    let mode: stat::Mode = options
        .perm
        .unwrap_or(stat::Mode::from_bits_truncate(0o770));

    let path = path.as_ref();
    nix::unistd::mkdir(path, mode)?;
    unistd::chown(path, options.owner, options.group)?;

    Ok(())
}

/// Recursively create a path with separately defined metadata for intermediate directories and the
/// final component in the path.
///
/// Returns `true` if the final directory was created. Otherwise `false` is returned and no changes
/// to the directory's metadata have been performed.
///
/// ```no_run
/// # use nix::sys::stat::Mode;
/// # use nix::unistd::{Gid, Uid};
/// # use proxmox::tools::fs::{create_path, CreateOptions};
/// # fn code() -> Result<(), anyhow::Error> {
/// create_path(
///     "/var/lib/mytool/wwwdata",
///     None,
///     Some(CreateOptions::new()
///         .perm(Mode::from_bits(0o777).unwrap())
///         .owner(Uid::from_raw(33))
///     ),
/// )?;
/// # Ok(())
/// # }
/// ```
pub fn create_path<P: AsRef<Path>>(
    path: P,
    intermediate_opts: Option<CreateOptions>,
    final_opts: Option<CreateOptions>,
) -> Result<bool, Error> {
    create_path_do(path.as_ref(), intermediate_opts, final_opts)
}

fn create_path_do(
    path: &Path,
    intermediate_opts: Option<CreateOptions>,
    final_opts: Option<CreateOptions>,
) -> Result<bool, Error> {
    use std::path::Component;

    let mut iter = path.components().peekable();
    let at: Fd = match iter.peek() {
        Some(Component::Prefix(_)) => bail!("illegal prefix path component encountered"),
        Some(Component::RootDir) => {
            let _ = iter.next();
            Fd::open(
                unsafe { CStr::from_bytes_with_nul_unchecked(b"/\0") },
                OFlag::O_DIRECTORY,
                stat::Mode::empty(),
            )?
        }
        Some(Component::CurDir) => {
            let _ = iter.next();
            Fd::cwd()
        }
        Some(Component::ParentDir) => {
            let _ = iter.next();
            Fd::open(
                unsafe { CStr::from_bytes_with_nul_unchecked(b"..\0") },
                OFlag::O_DIRECTORY,
                stat::Mode::empty(),
            )?
        }
        Some(Component::Normal(_)) => {
            // simply do not advance the iterator, heavy lifting happens in create_path_at_do()
            Fd::cwd()
        }
        None => bail!("create_path on empty path?"),
    };

    create_path_at_do(at, iter, intermediate_opts, final_opts)
}

fn create_path_at_do(
    mut at: Fd,
    mut iter: std::iter::Peekable<std::path::Components>,
    intermediate_opts: Option<CreateOptions>,
    final_opts: Option<CreateOptions>,
) -> Result<bool, Error> {
    let mut created = false;
    loop {
        use std::path::Component;

        match iter.next() {
            None => return Ok(created),

            Some(Component::ParentDir) => {
                at = Fd::openat(
                    &at,
                    unsafe { CStr::from_bytes_with_nul_unchecked(b"..\0") },
                    OFlag::O_DIRECTORY,
                    stat::Mode::empty(),
                )?;
            }

            Some(Component::Normal(path)) => {
                let opts = if iter.peek().is_some() {
                    intermediate_opts.as_ref()
                } else {
                    final_opts.as_ref()
                };

                // clippy bug?: from_bits_truncate is actually a const fn...
                #[allow(clippy::or_fun_call)]
                let mode = opts
                    .and_then(|o| o.perm)
                    .unwrap_or(stat::Mode::from_bits_truncate(0o755));

                created = match stat::mkdirat(at.as_raw_fd(), path, mode) {
                    Err(nix::Error::Sys(Errno::EEXIST)) => false,
                    Err(e) => return Err(e.into()),
                    Ok(_) => true,
                };
                at = Fd::openat(&at, path, OFlag::O_DIRECTORY, stat::Mode::empty())?;

                if let (true, Some(opts)) = (created, opts) {
                    if opts.owner.is_some() || opts.group.is_some() {
                        fchown(at.as_raw_fd(), opts.owner, opts.group)?;
                    }
                }
            }

            // impossible according to the docs:
            Some(_) => bail!("encountered unexpected special path component"),
        }
    }
}

#[test]
fn test_create_path() {
    create_path(
        "testdir/testsub/testsub2/testfinal",
        Some(CreateOptions::new().perm(stat::Mode::from_bits_truncate(0o755))),
        Some(
            CreateOptions::new()
                .owner(Uid::effective())
                .group(Gid::effective()),
        ),
    )
    .expect("expected create_path to work");
}

// /usr/include/linux/fs.h: #define BLKGETSIZE64 _IOR(0x12,114,size_t)
// return device size in bytes (u64 *arg)
nix::ioctl_read!(blkgetsize64, 0x12, 114, u64);

/// Return file or block device size
pub fn image_size(path: &Path) -> Result<u64, Error> {
    use std::os::unix::fs::FileTypeExt;

    let file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    let file_type = metadata.file_type();

    if file_type.is_block_device() {
        let mut size: u64 = 0;
        let res = unsafe { blkgetsize64(file.as_raw_fd(), &mut size) };

        if let Err(err) = res {
            bail!("blkgetsize64 failed for {:?} - {}", path, err);
        }
        Ok(size)
    } else if file_type.is_file() {
        Ok(metadata.len())
    } else {
        bail!(
            "image size failed - got unexpected file type {:?}",
            file_type
        );
    }
}
