use std::path::{Path, PathBuf};
use std::os::unix::io::AsRawFd;
use std::mem::MaybeUninit;
use std::fs::File;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::FromRawFd;
use std::ffi::{CStr, CString};

use anyhow::{bail, format_err, Error};
use nix::fcntl::OFlag;
use nix::sys::mman::{MapFlags, ProtFlags};
use nix::sys::stat::Mode;
use nix::errno::Errno;

use proxmox::tools::fs::{create_path, CreateOptions};
use proxmox::tools::mmap::Mmap;
use proxmox::sys::error::SysError;

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

const fn up_to_page_size(n: usize) ->  usize {
    // FIXME: use sysconf(_SC_PAGE_SIZE)
    (n + 4095) & !4095
}

fn mmap_file<T: Init>(file: &mut File, initialize: bool) -> Result<Mmap<T>, Error> {
    // map it as MaybeUninit
    let mut mmap: Mmap<MaybeUninit<T>> = unsafe {
        Mmap::map_fd(
            file.as_raw_fd(),
            0, 1,
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
            MapFlags::MAP_SHARED | MapFlags::MAP_NORESERVE | MapFlags::MAP_POPULATE,
        )?
    };

    if initialize {
        Init::initialize(&mut mmap[0]);
    }

    Ok(unsafe { std::mem::transmute(mmap) })
}

impl <T: Sized + Init> SharedMemory<T> {

    pub fn open(path: &Path, options: CreateOptions) -> Result<Self, Error> {

        let size = std::mem::size_of::<T>();
        let up_size = up_to_page_size(size);

        if size != up_size {
            bail!("SharedMemory::open {:?} failed - data size {} is not a multiple of 4096", path, size);
        }

        let mmap = Self::open_shmem(path, options)?;

        Ok(Self { mmap })
    }

    pub fn open_shmem<P: AsRef<Path>>(
        path: P,
        options: CreateOptions,
    ) -> Result<Mmap<T>, Error> {
        let path = path.as_ref();

        let dir_name = path
            .parent()
            .ok_or_else(|| format_err!("bad path {:?}", path))?
            .to_owned();

        let statfs = nix::sys::statfs::statfs(&dir_name)?;
        if statfs.filesystem_type() != nix::sys::statfs::TMPFS_MAGIC {
            bail!("path {:?} is not on tmpfs", dir_name);
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
                    // fall thrue -  try to create the file
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
            Ok(_rc) => return Ok(mmap),
            // if someone else was faster, open the existing file:
            Err(nix::Error::Sys(Errno::EEXIST)) =>  {
                // if opening fails again now, we'll just error...
                match nix::fcntl::open(path, oflag, Mode::empty()) {
                    Ok(fd) => {
                        let mut file = unsafe { File::from_raw_fd(fd) };
                        let mmap = mmap_file(&mut file, false)?;
                        return Ok(mmap);
                    }
                    Err(err) => bail!("open {:?} failed - {}", path, err),
                };
            }
            Err(err) =>  return Err(err.into()),
        }
    }

    pub fn data(&self) -> &T {
        &self.mmap[0]
    }

    pub fn data_mut(&mut self) -> &mut T {
        &mut self.mmap[0]
    }

}

#[cfg(test)]
mod test {

    use super::*;

    use std::sync::atomic::AtomicU64;

    #[derive(Debug)]
    #[repr(C)]
    struct TestData {
        count: u64,
        value1: u64,
        value2: u64,
    }

    impl Init for TestData {

        fn initialize(this: &mut MaybeUninit<Self>) {
            this.write(Self {
                count: 0,
                value1: 0xffff_ffff_ffff_0000,
                value2: 0x0000_ffff_ffff_ffff,
            });
        }
    }

    struct SingleMutexData {
        data: SharedMutex<TestData>,
        padding: [u8; 4096 - 64],
    }

    impl Init for SingleMutexData {
        fn initialize(this: &mut MaybeUninit<Self>) {
            let me = unsafe { &mut *this.as_mut_ptr() };
            let data: &mut MaybeUninit<SharedMutex<TestData>> =  unsafe { std::mem::transmute(&mut me.data) };
            Init::initialize(data);
        }
    }

    #[test]
    fn test_shared_memory_mutex() -> Result<(), Error> {

        create_path("/run/proxmox-shmem", None, None);

        let shared: SharedMemory<SingleMutexData> =
            SharedMemory::open(Path::new("/run/proxmox-shmem/test.shm"), CreateOptions::new())?;

        let mut guard = shared.data().data.lock();
        println!("DATA {:?}", *guard);
        guard.count += 1;
        println!("DATA {:?}", *guard);

        //unimplemented!();

        Ok(())
    }

    #[derive(Debug)]
    #[repr(C)]
    struct MultiMutexData {
        acount: AtomicU64,
        block1: SharedMutex<TestData>,
        block2: SharedMutex<TestData>,
        padding: [u8; 4096 - 136],
    }

    impl Init for MultiMutexData {
        fn initialize(this: &mut MaybeUninit<Self>) {
            let me = unsafe { &mut *this.as_mut_ptr() };

            let block1: &mut MaybeUninit<SharedMutex<TestData>> =  unsafe { std::mem::transmute(&mut me.block1) };
            Init::initialize(block1);

            let block2: &mut MaybeUninit<SharedMutex<TestData>> =  unsafe { std::mem::transmute(&mut me.block2) };
            Init::initialize(block2);
        }
    }

    #[test]
    fn test_shared_memory_multi_mutex() -> Result<(), Error> {

        let shared: SharedMemory<MultiMutexData> =
            SharedMemory::open(Path::new("/run/proxmox-shmem/test3.shm"), CreateOptions::new())?;

        let mut guard = shared.data().block1.lock();
        println!("BLOCK1 {:?}", *guard);
        guard.count += 1;

        let mut guard = shared.data().block2.lock();
        println!("BLOCK2 {:?}", *guard);
        guard.count += 2;

        unimplemented!();
    }

}
