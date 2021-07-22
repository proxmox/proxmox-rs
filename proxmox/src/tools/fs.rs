//! File related utilities such as `replace_file`.

use std::ffi::CStr;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, format_err, Error};
use nix::errno::Errno;
use nix::fcntl::OFlag;
use nix::sys::stat;
use nix::unistd::{self, Gid, Uid};
use nix::NixPath;
use serde_json::Value;

use crate::sys::error::{SysError, SysResult};
use crate::sys::timer;
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

/// Read the entire contents of a file into a bytes vector if the file exists
///
/// Same as file_get_contents(), but returns 'Ok(None)' instead of
/// 'Err' if the file dose not exist.
pub fn file_get_optional_contents<P: AsRef<Path>>(path: P) -> Result<Option<Vec<u8>>, Error> {
    let path = path.as_ref();

    match std::fs::read(path) {
        Ok(content) => Ok(Some(content)),
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                Ok(None)
            } else {
                bail!("unable to read '{:?}' - {}", path, err);
            }
        }
    }
}

/// Read the entire contents of a file into a String
///
/// This basically call ``std::fs::read_to_string``, but provides more elaborate
/// error messages including the path.
pub fn file_read_string<P: AsRef<Path>>(path: P) -> Result<String, Error> {
    let path = path.as_ref();

    std::fs::read_to_string(path).map_err(|err| format_err!("unable to read {:?} - {}", path, err))
}

/// Read the entire contents of a file into a String if the file exists
///
/// Same as file_read_string(), but returns 'Ok(None)' instead of
/// 'Err' if the file dose not exist.
pub fn file_read_optional_string<P: AsRef<Path>>(path: P) -> Result<Option<String>, Error> {
    let path = path.as_ref();

    match std::fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                Ok(None)
            } else {
                bail!("unable to read '{:?}' - {}", path, err);
            }
        }
    }
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

/// Takes a Path and CreateOptions, creates a tmpfile from it and returns
/// a RawFd and PathBuf for it
pub fn make_tmp_file<P: AsRef<Path>>(
    path: P,
    options: CreateOptions,
) -> Result<(File, PathBuf), Error> {
    let path = path.as_ref();

    // use mkstemp here, because it works with different processes, threads, even tokio tasks
    let mut template = path.to_owned();
    template.set_extension("tmp_XXXXXX");
    let (file, tmp_path) = match unistd::mkstemp(&template) {
        Ok((fd, path)) => (unsafe { File::from_raw_fd(fd) }, path),
        Err(err) => bail!("mkstemp {:?} failed: {}", template, err),
    };

    // clippy bug?: from_bits_truncate is actually a const fn...
    #[allow(clippy::or_fun_call)]
    let mode: stat::Mode = options
        .perm
        .unwrap_or(stat::Mode::from_bits_truncate(0o644));

    if let Err(err) = stat::fchmod(file.as_raw_fd(), mode) {
        let _ = unistd::unlink(&tmp_path);
        bail!("fchmod {:?} failed: {}", tmp_path, err);
    }

    if options.owner.is_some() || options.group.is_some() {
        if let Err(err) = fchown(file.as_raw_fd(), options.owner, options.group) {
            let _ = unistd::unlink(&tmp_path);
            bail!("fchown {:?} failed: {}", tmp_path, err);
        }
    }

    Ok((file, tmp_path))
}

/// Atomically replace a file.
///
/// This first creates a temporary file and then rotates it in place.
pub fn replace_file<P: AsRef<Path>>(
    path: P,
    data: &[u8],
    options: CreateOptions,
) -> Result<(), Error> {
    let (fd, tmp_path) = make_tmp_file(&path, options)?;

    let mut file = unsafe { File::from_raw_fd(fd.into_raw_fd()) };

    if let Err(err) = file.write_all(data) {
        let _ = unistd::unlink(&tmp_path);
        bail!("write failed: {}", err);
    }

    if let Err(err) = std::fs::rename(&tmp_path, &path) {
        let _ = unistd::unlink(&tmp_path);
        bail!(
            "Atomic rename failed for file {:?} - {}",
            path.as_ref(),
            err
        );
    }

    Ok(())
}

/// Like open(2), but allows setting initial data, perm, owner and group
///
/// Since we need to initialize the file, we also need a solid slow
/// path where we create the file. In order to avoid races, we create
/// it in a temporary location and rotate it in place.
pub fn atomic_open_or_create_file<P: AsRef<Path>>(
    path: P,
    mut oflag: OFlag,
    initial_data: &[u8],
    options: CreateOptions,
) -> Result<File, Error> {
    let path = path.as_ref();

    if oflag.contains(OFlag::O_TMPFILE) {
        bail!("open {:?} failed - unsupported OFlag O_TMPFILE", path);
    }

    oflag.remove(OFlag::O_CREAT); // we want to handle CREAT ourselfes

    // Note: 'mode' is ignored, because oflag does not contain O_CREAT or O_TMPFILE
    match nix::fcntl::open(path, oflag, stat::Mode::empty()) {
        Ok(fd) => return Ok(unsafe { File::from_raw_fd(fd) }),
        Err(err) => {
            if err.not_found() {
                // fall thrue -  try to create the file
            } else {
                bail!("open {:?} failed - {}", path, err);
            }
        }
    }

    let (mut file, temp_file_name) = make_tmp_file(path, options)?;

    if !initial_data.is_empty() {
        file.write_all(initial_data).map_err(|err| {
            let _ = nix::unistd::unlink(&temp_file_name);
            format_err!(
                "writing initial data to {:?} failed - {}",
                temp_file_name,
                err,
            )
        })?;
    }

    // rotate the file into place, but use `RENAME_NOREPLACE`, so in case 2 processes race against
    // the initialization, the first one wins!
    let rename_result = temp_file_name.with_nix_path(|c_file_name| {
        path.with_nix_path(|new_path| unsafe {
            // This also works on file systems which don't support hardlinks (eg. vfat)
            match Errno::result(libc::renameat2(
                libc::AT_FDCWD,
                c_file_name.as_ptr(),
                libc::AT_FDCWD,
                new_path.as_ptr(),
                libc::RENAME_NOREPLACE,
            )) {
                Err(nix::Error::Sys(Errno::EINVAL)) => (), // dumb file system, try `link`+`unlink`
                other => return other,
            };
            // but some file systems don't support `RENAME_NOREPLACE`
            // so we just use `link` + `unlink` instead
            let result = Errno::result(libc::link(c_file_name.as_ptr(), new_path.as_ptr()));
            let _ = libc::unlink(c_file_name.as_ptr());
            result
        })
    });

    match rename_result {
        Ok(Ok(Ok(_))) => Ok(file),
        Ok(Ok(Err(err))) => {
            // if another process has already raced ahead and created
            // the file, let's just open theirs instead:
            let _ = nix::unistd::unlink(&temp_file_name);

            if err.already_exists() {
                match nix::fcntl::open(path, oflag, stat::Mode::empty()) {
                    Ok(fd) => Ok(unsafe { File::from_raw_fd(fd) }),
                    Err(err) => bail!("open {:?} failed - {}", path, err),
                }
            } else {
                bail!(
                    "failed to move file at {:?} into place at {:?} - {}",
                    temp_file_name,
                    path,
                    err
                );
            }
        }
        Ok(Err(err)) => {
            let _ = nix::unistd::unlink(&temp_file_name);
            bail!("with_nix_path {:?} failed - {}", path, err);
        }
        Err(err) => {
            let _ = nix::unistd::unlink(&temp_file_name);
            bail!("with_nix_path {:?} failed - {}", temp_file_name, err);
        }
    }
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

/// Create a file lock using fntl. This function allows you to specify
/// a timeout if you want to avoid infinite blocking.
///
/// With timeout set to 0, non-blocking mode is used and the function
/// will fail immediately if the lock can't be acquired.
pub fn lock_file<F: AsRawFd>(
    file: &mut F,
    exclusive: bool,
    timeout: Option<Duration>,
) -> Result<(), io::Error> {
    let lockarg = if exclusive {
        nix::fcntl::FlockArg::LockExclusive
    } else {
        nix::fcntl::FlockArg::LockShared
    };

    let timeout = match timeout {
        None => {
            nix::fcntl::flock(file.as_raw_fd(), lockarg).into_io_result()?;
            return Ok(());
        }
        Some(t) => t,
    };

    if timeout.as_nanos() == 0 {
        let lockarg = if exclusive {
            nix::fcntl::FlockArg::LockExclusiveNonblock
        } else {
            nix::fcntl::FlockArg::LockSharedNonblock
        };
        nix::fcntl::flock(file.as_raw_fd(), lockarg).into_io_result()?;
        return Ok(());
    }

    // unblock the timeout signal temporarily
    let _sigblock_guard = timer::unblock_timeout_signal();

    // setup a timeout timer
    let mut timer = timer::Timer::create(
        timer::Clock::Realtime,
        timer::TimerEvent::ThisThreadSignal(timer::SIGTIMEOUT),
    )?;

    timer.arm(
        timer::TimerSpec::new()
            .value(Some(timeout))
            .interval(Some(Duration::from_millis(10))),
    )?;

    nix::fcntl::flock(file.as_raw_fd(), lockarg).into_io_result()?;
    Ok(())
}

/// Open or create a lock file (append mode). Then try to
/// acquire a lock using `lock_file()`.
pub fn open_file_locked<P: AsRef<Path>>(
    path: P,
    timeout: Duration,
    exclusive: bool,
    options: CreateOptions,
) -> Result<File, Error> {
    let path = path.as_ref();

    let mut file = atomic_open_or_create_file(
        path,
        OFlag::O_RDWR | OFlag::O_CLOEXEC | OFlag::O_APPEND,
        &[],
        options,
    )?;

    match lock_file(&mut file, exclusive, Some(timeout)) {
        Ok(_) => Ok(file),
        Err(err) => bail!("Unable to acquire lock {:?} - {}", path, err),
    }
}
