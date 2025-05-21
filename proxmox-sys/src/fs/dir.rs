use std::ffi::OsString;
use std::os::fd::FromRawFd;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::io::{AsRawFd, OwnedFd};
use std::path::{Path, PathBuf};

use anyhow::{bail, format_err, Error};
use nix::errno::Errno;
use nix::fcntl::OFlag;
use nix::sys::stat;
use nix::unistd;

use proxmox_lang::try_block;

use crate::fs::{fchown, CreateOptions};

/// The default list of [`OFlag`]'s we want to use when opening directories. Besides ensuring that
/// the FD indeed points to a directory we also must ensure that it gets closed on exec to avoid
/// leaking a open FD to a child process.
pub(crate) const DIR_FLAGS: OFlag = OFlag::O_DIRECTORY.union(OFlag::O_CLOEXEC);

/// Creates directory at the provided path with specified ownership.
///
/// Errors if the directory already exists.
pub fn create_dir<P: AsRef<Path>>(path: P, options: CreateOptions) -> Result<(), Error> {
    // clippy bug?: from_bits_truncate is actually a const fn...
    #[allow(clippy::or_fun_call)]
    let mode: stat::Mode = options
        .perm
        .unwrap_or(stat::Mode::from_bits_truncate(0o750));

    let path = path.as_ref();

    nix::unistd::mkdir(path, mode)
        .map_err(|err| format_err!("unable to create directory {path:?} - {err}"))?;

    unistd::chown(path, options.owner, options.group)
        .map_err(|err| format_err!("unable to set ownership for directory {path:?} - {err}"))?;

    Ok(())
}

/// Ensure a directory exists.
///
/// Like [create_dir], but does not fail if the directory already exists.
///
/// Directory permissions are verified and raise an error if enforce_permissions is set.
pub fn ensure_dir_exists<P: AsRef<Path>>(
    path: P,
    options: &CreateOptions,
    enforce_permissions: bool,
) -> Result<(), Error> {
    let uid = options.owner;
    let gid = options.group;

    let mode: stat::Mode = options
        .perm
        .unwrap_or(stat::Mode::from_bits_truncate(0o750));

    let path = path.as_ref();

    match nix::unistd::mkdir(path, mode) {
        Ok(()) => (),
        Err(nix::errno::Errno::EEXIST) => {
            if enforce_permissions {
                return options.check(path);
            } else if let Err(err) = options.check(path) {
                log::error!("{err}");
            }
        }
        Err(err) => bail!("unable to create directory {path:?} - {err}",),
    }

    let fd = nix::fcntl::open(path, DIR_FLAGS, stat::Mode::empty())
        .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) })
        .map_err(|err| format_err!("unable to open created directory {path:?} - {err}"))?;
    // umask defaults to 022 so make sure the mode is fully honowed:
    nix::sys::stat::fchmod(fd.as_raw_fd(), mode)
        .map_err(|err| format_err!("unable to set mode for directory {path:?} - {err}"))?;
    nix::unistd::fchown(fd.as_raw_fd(), uid, gid)
        .map_err(|err| format_err!("unable to set ownership for directory {path:?} - {err}"))?;

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
/// # use proxmox_sys::fs::{create_path, CreateOptions};
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
    let at: OwnedFd = match iter.peek() {
        Some(Component::Prefix(_)) => bail!("illegal prefix path component encountered"),
        Some(Component::RootDir) => {
            let _ = iter.next();
            crate::fd::open(c"/", DIR_FLAGS, stat::Mode::empty())?
        }
        Some(Component::CurDir) => {
            let _ = iter.next();
            crate::fd::cwd()?
        }
        Some(Component::ParentDir) => {
            let _ = iter.next();
            crate::fd::open(c"..", DIR_FLAGS, stat::Mode::empty())?
        }
        Some(Component::Normal(_)) => {
            // simply do not advance the iterator, heavy lifting happens in create_path_at_do()
            crate::fd::cwd()?
        }
        None => bail!("create_path on empty path?"),
    };

    create_path_at_do(at, iter, intermediate_opts, final_opts)
}

fn create_path_at_do(
    mut at: OwnedFd,
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
                at = crate::fd::openat(&at, c"..", DIR_FLAGS, stat::Mode::empty())?;
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

                created = match stat::mkdirat(Some(at.as_raw_fd()), path, mode) {
                    Err(Errno::EEXIST) => false,
                    Err(e) => return Err(e.into()),
                    Ok(_) => true,
                };
                at = crate::fd::openat(&at, path, DIR_FLAGS, stat::Mode::empty())?;

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

///  Create a temporary directory.
///
/// `directory` determines where the temporary directory will be created. For instance, if
/// `directory` is `/tmp`, on success the function will return a path in the style of
/// `/tmp/tmp_XXXXXX`, where X stands for a random string, ensuring that the path is unique.
///
/// By default, the created directory has `0o700` permissions. If this is not desired, custom
/// [`CreateOptions`] can be passed via the `options` parameter.
pub fn make_tmp_dir<P: AsRef<Path>>(
    directory: P,
    options: Option<CreateOptions>,
) -> Result<PathBuf, Error> {
    let template = directory.as_ref().join("tmp_XXXXXX");

    let mut template = template.into_os_string().into_vec();
    // Push NULL byte so that we have a proper NULL-terminated string
    template.push(0);

    use std::os::raw::c_char;
    let returned_buffer = unsafe { libc::mkdtemp(template.as_mut_ptr() as *mut c_char) };

    // Check errno immediately, so that nothing else can overwrite it.
    let err = std::io::Error::last_os_error();

    if returned_buffer.is_null() {
        return Err(err.into());
    }

    template.pop(); // drop terminating nul byte
    let path = PathBuf::from(OsString::from_vec(template));

    if let Some(options) = options {
        if let Err(err) = try_block!({
            let mut fd = crate::fd::open(&path, DIR_FLAGS, stat::Mode::empty())?;
            options.apply_to(&mut fd, &path)?;
            Ok::<(), Error>(())
        }) {
            if let Err(err) = std::fs::remove_dir(&path) {
                log::error!("could not clean up temporary directory at {path:?}: {err}")
            }
            bail!("could not apply create options to new temporary directory: {err}");
        }
    }

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_path() {
        create_path(
            "testdir/testsub/testsub2/testfinal",
            Some(CreateOptions::new().perm(stat::Mode::from_bits_truncate(0o755))),
            Some(
                CreateOptions::new()
                    .owner(nix::unistd::Uid::effective())
                    .group(nix::unistd::Gid::effective()),
            ),
        )
        .expect("expected create_path to work");
    }

    #[test]
    fn test_make_tmp_dir() -> Result<(), Error> {
        let options = CreateOptions::new()
            .owner(unistd::Uid::effective())
            .group(unistd::Gid::effective())
            .perm(stat::Mode::from_bits_truncate(0o755));

        let path = make_tmp_dir("/tmp", Some(options))?;

        assert!(path.exists());
        assert!(path.is_dir());

        std::fs::remove_dir_all(&path)?;

        Ok(())
    }
}
