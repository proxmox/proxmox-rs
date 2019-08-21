use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::path::Path;

use failure::{bail, format_err, Error};
use nix::sys::stat;
use nix::unistd::{self, Gid, Uid};
use serde_json::Value;

use super::try_block;

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

/// Atomically write a file
///
/// We first create a temporary file, which is then renamed.
pub fn file_set_contents<P: AsRef<Path>>(
    path: P,
    data: &[u8],
    perm: Option<stat::Mode>,
) -> Result<(), Error> {
    file_set_contents_full(path, data, perm, None, None)
}

/// Atomically write a file with owner and group
pub fn file_set_contents_full<P: AsRef<Path>>(
    path: P,
    data: &[u8],
    perm: Option<stat::Mode>,
    owner: Option<Uid>,
    group: Option<Gid>,
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

    let mode: stat::Mode = perm.unwrap_or(stat::Mode::from(
        stat::Mode::S_IRUSR | stat::Mode::S_IWUSR | stat::Mode::S_IRGRP | stat::Mode::S_IROTH,
    ));

    if perm != None {
        if let Err(err) = stat::fchmod(fd, mode) {
            let _ = unistd::unlink(tmp_path);
            bail!("fchmod {:?} failed: {}", tmp_path, err);
        }
    }

    if owner != None || group != None {
        if let Err(err) = fchown(fd, owner, group) {
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

/// Change ownership of an open file handle
pub fn fchown(fd: RawFd, owner: Option<Uid>, group: Option<Gid>) -> Result<(), Error> {
    // According to the POSIX specification, -1 is used to indicate that owner and group
    // are not to be changed.  Since uid_t and gid_t are unsigned types, we have to wrap
    // around to get -1 (copied fron nix crate).
    let uid = owner
        .map(Into::into)
        .unwrap_or((0 as libc::uid_t).wrapping_sub(1));
    let gid = group
        .map(Into::into)
        .unwrap_or((0 as libc::gid_t).wrapping_sub(1));

    let res = unsafe { libc::fchown(fd, uid, gid) };
    nix::errno::Errno::result(res)?;

    Ok(())
}

/// Creates directory at the provided path with specified ownership
///
/// Simply returns if the directory already exists.
pub fn create_dir_chown<P: AsRef<Path>>(
    path: P,
    perm: Option<stat::Mode>,
    owner: Option<Uid>,
    group: Option<Gid>,
) -> Result<(), nix::Error> {
    let mode: stat::Mode = perm.unwrap_or(stat::Mode::from_bits_truncate(0o770));

    let path = path.as_ref();

    match nix::unistd::mkdir(path, mode) {
        Ok(()) => {}
        Err(nix::Error::Sys(nix::errno::Errno::EEXIST)) => {
            return Ok(());
        }
        err => return err,
    }

    unistd::chown(path, owner, group)?;

    Ok(())
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
