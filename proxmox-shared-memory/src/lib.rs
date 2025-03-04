#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

use std::ffi::{CStr, CString};
use std::fs::File;
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::AsRawFd;
use std::os::unix::io::FromRawFd;
use std::path::Path;

use anyhow::{bail, format_err, Error};
use nix::errno::Errno;
use nix::fcntl::OFlag;
use nix::sys::mman::{MapFlags, ProtFlags};
use nix::sys::stat::Mode;

use proxmox_sys::error::SysError;
use proxmox_sys::fs::CreateOptions;
use proxmox_sys::mmap::Mmap;

mod raw_shared_mutex;

mod shared_mutex;
pub use shared_mutex::*;

/// Data inside SharedMemory need to implement this trait
///
/// IMPORTANT: Please use #[repr(C)] for all types implementing this
pub trait Init: Sized {
    /// Make sure the data struicture is initialized. This is called
    /// after mapping into shared memory. The caller makes sure that
    /// no other process run this at the same time.
    fn initialize(this: &mut MaybeUninit<Self>);

    /// Check if the data has the correct format
    fn check_type_magic(_this: &MaybeUninit<Self>) -> Result<(), Error> {
        Ok(())
    }
}

/// Memory mapped shared memory region
///
/// This allows access to same memory region for multiple
/// processes. You should only use atomic types from 'std::sync::atomic', or
/// protect the data with [SharedMutex].
///
/// SizeOf(T) needs to be a multiple of 4096 (the page size).
pub struct SharedMemory<T> {
    mmap: Mmap<T>,
}

const fn up_to_page_size(n: usize) -> usize {
    // FIXME: use sysconf(_SC_PAGE_SIZE)
    (n + 4095) & !4095
}

fn mmap_file<T: Init>(file: &mut File, initialize: bool) -> Result<Mmap<T>, Error> {
    // map it as MaybeUninit
    let mut mmap: Mmap<MaybeUninit<T>> = unsafe {
        Mmap::map_fd(
            file.as_raw_fd(),
            0,
            1,
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
            MapFlags::MAP_SHARED | MapFlags::MAP_NORESERVE | MapFlags::MAP_POPULATE,
        )?
    };

    if initialize {
        Init::initialize(&mut mmap[0]);
    }

    match Init::check_type_magic(&mmap[0]) {
        Ok(()) => (),
        Err(err) => bail!("detected wrong types in mmaped files: {}", err),
    }

    Ok(unsafe { std::mem::transmute::<Mmap<MaybeUninit<T>>, Mmap<T>>(mmap) })
}

impl<T: Sized + Init> SharedMemory<T> {
    pub fn open(path: &Path, options: CreateOptions) -> Result<Self, Error> {
        let size = std::mem::size_of::<T>();
        let up_size = up_to_page_size(size);

        if size != up_size {
            bail!(
                "SharedMemory::open {:?} failed - data size {} is not a multiple of 4096",
                path,
                size
            );
        }

        let mmap = Self::open_shmem(path, options)?;

        Ok(Self { mmap })
    }

    pub fn open_shmem<P: AsRef<Path>>(path: P, options: CreateOptions) -> Result<Mmap<T>, Error> {
        let path = path.as_ref();

        let dir_name = path
            .parent()
            .ok_or_else(|| format_err!("bad path {:?}", path))?
            .to_owned();

        if !dir_name.ends_with("shmemtest") {
            let statfs = nix::sys::statfs::statfs(&dir_name)?;
            if statfs.filesystem_type() != nix::sys::statfs::TMPFS_MAGIC {
                bail!("path {:?} is not on tmpfs", dir_name);
            }
        }

        let oflag = OFlag::O_RDWR | OFlag::O_CLOEXEC;

        // Try to open existing file
        match nix::fcntl::open(path, oflag, Mode::empty()) {
            Ok(fd) => {
                let mut file = unsafe { File::from_raw_fd(fd) };
                let mmap = mmap_file(&mut file, false)?;
                return Ok(mmap);
            }
            Err(err) => {
                if err.not_found() {
                    // fall true -  try to create the file
                } else {
                    bail!("open {:?} failed - {}", path, err);
                }
            }
        }

        // create temporary file using O_TMPFILE
        let mut file = match nix::fcntl::open(&dir_name, oflag | OFlag::O_TMPFILE, Mode::empty()) {
            Ok(fd) => {
                let mut file = unsafe { File::from_raw_fd(fd) };
                options.apply_to(&mut file, &dir_name)?;
                file
            }
            Err(err) => {
                bail!("open tmpfile in {:?} failed - {}", dir_name, err);
            }
        };

        let size = std::mem::size_of::<T>();
        let size = up_to_page_size(size);

        nix::unistd::ftruncate(file.as_raw_fd(), size as i64)?;

        // link the file into place:
        let proc_path = format!("/proc/self/fd/{}\0", file.as_raw_fd());
        let proc_path = unsafe { CStr::from_bytes_with_nul_unchecked(proc_path.as_bytes()) };

        let mmap = mmap_file(&mut file, true)?;

        let res = {
            let path = CString::new(path.as_os_str().as_bytes())?;
            Errno::result(unsafe {
                libc::linkat(
                    -1,
                    proc_path.as_ptr(),
                    libc::AT_FDCWD,
                    path.as_ptr(),
                    libc::AT_SYMLINK_FOLLOW,
                )
            })
        };

        drop(file); // no longer required

        match res {
            Ok(_rc) => Ok(mmap),
            // if someone else was faster, open the existing file:
            Err(Errno::EEXIST) => {
                // if opening fails again now, we'll just error...
                match nix::fcntl::open(path, oflag, Mode::empty()) {
                    Ok(fd) => {
                        let mut file = unsafe { File::from_raw_fd(fd) };
                        let mmap = mmap_file(&mut file, false)?;
                        Ok(mmap)
                    }
                    Err(err) => bail!("open {:?} failed - {}", path, err),
                }
            }
            Err(err) => Err(err.into()),
        }
    }

    pub fn data(&self) -> &T {
        &self.mmap[0]
    }

    pub fn data_mut(&mut self) -> &mut T {
        &mut self.mmap[0]
    }
}

/// Helper to initialize nested data
///
/// # Safety
///
/// This calls `Init::initialize`, it is up to the user to ensure this is safe. The value should
/// not have been initialized at this point.
pub unsafe fn initialize_subtype<T: Init>(this: &mut T) {
    let data: &mut MaybeUninit<T> = unsafe { std::mem::transmute(this) };
    Init::initialize(data);
}

/// Helper to call 'check_type_magic' for nested data
///
/// # Safety
///
/// This calls `Init::check_type_magic`, it is up to the user to ensure this is safe.
pub unsafe fn check_subtype<T: Init>(this: &T) -> Result<(), Error> {
    let data: &MaybeUninit<T> = unsafe { std::mem::transmute(this) };
    Init::check_type_magic(data)
}
