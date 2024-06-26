use std::path::PathBuf;
use std::{mem::MaybeUninit, sync::Arc, thread::spawn};

use anyhow::Error;
use nix::fcntl::OFlag;
use nix::sys::stat::Mode;
use nix::unistd::mkdir;
use proxmox_shared_memory::{check_subtype, initialize_subtype, Init, SharedMemory, SharedMutex};
use proxmox_sys::fs::CreateOptions;

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
    _padding: [u8; 4096 - 64 - 8],
}

impl Init for SingleMutexData {
    fn initialize(this: &mut MaybeUninit<Self>) {
        unsafe {
            let me = &mut *this.as_mut_ptr();
            initialize_subtype(&mut me.data);
        }
    }

    fn check_type_magic(this: &MaybeUninit<Self>) -> Result<(), Error> {
        unsafe {
            let me = &*this.as_ptr();
            check_subtype(&me.data)
        }
    }
}

#[derive(Debug)]
#[repr(C)]
struct MultiMutexData {
    acount: AtomicU64,
    block1: SharedMutex<TestData>,
    block2: SharedMutex<TestData>,
    padding: [u8; 4096 - 136 - 16],
}

impl Init for MultiMutexData {
    fn initialize(this: &mut MaybeUninit<Self>) {
        unsafe {
            let me = &mut *this.as_mut_ptr();
            initialize_subtype(&mut me.block1);
            initialize_subtype(&mut me.block2);
        }
    }

    fn check_type_magic(this: &MaybeUninit<Self>) -> Result<(), Error> {
        unsafe {
            let me = &*this.as_ptr();
            check_subtype(&me.block1)?;
            check_subtype(&me.block2)?;
            Ok(())
        }
    }
}

fn create_test_dir(filename: &str) -> Option<PathBuf> {
    let test_dir: String = env!("CARGO_TARGET_TMPDIR").to_string();

    let mut path = PathBuf::from(&test_dir);
    path.push("shmemtest");
    let _ = mkdir(&path, Mode::S_IRWXU);
    path.push(filename);

    let oflag = OFlag::O_RDWR | OFlag::O_CLOEXEC;

    // check for O_TMPFILE support
    if let Err(nix::errno::Errno::EOPNOTSUPP) = nix::fcntl::open(
        path.parent().unwrap(),
        oflag | OFlag::O_TMPFILE,
        Mode::empty(),
    ) {
        return None;
    }

    Some(path)
}
#[test]
fn test_shared_memory_mutex() -> Result<(), Error> {
    let path = match create_test_dir("data1.shm") {
        None => {
            return Ok(()); // no O_TMPFILE support, can't run test
        }
        Some(path) => path,
    };

    let shared: SharedMemory<SingleMutexData> = SharedMemory::open(&path, CreateOptions::new())?;

    let shared = Arc::new(shared);

    let start = shared.data().data.lock().count;

    let threads: Vec<_> = (0..100)
        .map(|_| {
            let shared = shared.clone();
            spawn(move || {
                let mut guard = shared.data().data.lock();
                println!("DATA {:?}", *guard);
                guard.count += 1;
                println!("DATA {:?}", *guard);
            })
        })
        .collect();

    for thread in threads {
        thread.join().unwrap();
    }

    let end = shared.data().data.lock().count;

    assert_eq!(end - start, 100);

    Ok(())
}

#[test]
fn test_shared_memory_multi_mutex() -> Result<(), Error> {
    let path = match create_test_dir("data2.shm") {
        None => {
            return Ok(()); // no O_TMPFILE support, can't run test
        }
        Some(path) => path,
    };
    let shared: SharedMemory<MultiMutexData> = SharedMemory::open(&path, CreateOptions::new())?;

    let shared = Arc::new(shared);

    let start1 = shared.data().block1.lock().count;
    let start2 = shared.data().block2.lock().count;

    let threads: Vec<_> = (0..100)
        .map(|_| {
            let shared = shared.clone();
            spawn(move || {
                let mut guard = shared.data().block1.lock();
                println!("BLOCK1 {:?}", *guard);
                guard.count += 1;
                let mut guard = shared.data().block2.lock();
                println!("BLOCK2 {:?}", *guard);
                guard.count += 2;
            })
        })
        .collect();

    for thread in threads {
        thread.join().unwrap();
    }

    let end1 = shared.data().block1.lock().count;
    assert_eq!(end1 - start1, 100);

    let end2 = shared.data().block2.lock().count;
    assert_eq!(end2 - start2, 200);

    Ok(())
}
