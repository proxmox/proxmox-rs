use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::{bail, Error};
use hyper::http::method::Method;
use nix::sys::mman::MsFlags;
use nix::sys::stat::Mode;
use nix::unistd::User;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use proxmox_shared_memory::{Init, SharedMemory};
use proxmox_sys::fs::CreateOptions;

const MEMORY_PAGE_SIZE: usize = 4096;
/// Generated via openssl::sha::sha256(b"Proxmox shared request counters v1.0")[0..8]
const PROXMOX_SHARED_REQUEST_COUNTERS_1_0: [u8; 8] = [224, 110, 88, 252, 26, 77, 180, 5];

#[repr(C, align(32))]
#[derive(Default)]
/// AtomicU64 aligned to the half default cache line size of 64-bytes.
struct AlignedAtomic(AtomicU64);

#[repr(C, align(32))]
#[derive(Default, PartialEq)]
/// Mmapped file magic number aligned to half the default cache line size of 64-bytes.
/// Facilitates the padding size calculation.
struct AlignedMagic([u8; 8]);

#[repr(C)]
#[derive(Default)]
// Ordering is chosen to bundle frequently expected counter updates with less
// fequent ones. Ideally each counter would live in it's own cache line, but
// that requires double the memory.
struct RequestCounters {
    // request count
    get: AlignedAtomic,
    delete: AlignedAtomic,
    put: AlignedAtomic,
    head: AlignedAtomic,
    post: AlignedAtomic,
    // traffic in bytes
    upload: AlignedAtomic,
    download: AlignedAtomic,
}

impl Init for RequestCounters {
    fn initialize(this: &mut MaybeUninit<Self>) {
        // safety: RequestCounters contains simple data types with no internal references.
        this.write(RequestCounters::default());
    }
}

impl RequestCounters {
    /// Increment the counter for given method, following the provided memory ordering constrains.
    ///
    /// Returns the previously stored value.
    pub fn increment(&self, method: Method, ordering: Ordering) -> u64 {
        match method {
            Method::DELETE => self.delete.0.fetch_add(1, ordering),
            Method::GET => self.get.0.fetch_add(1, ordering),
            Method::HEAD => self.head.0.fetch_add(1, ordering),
            Method::POST => self.post.0.fetch_add(1, ordering),
            Method::PUT => self.put.0.fetch_add(1, ordering),
            _ => 0,
        }
    }

    /// Load current counter state for given method, following the provided memory ordering constrains
    pub fn load(&self, method: Method, ordering: Ordering) -> u64 {
        match method {
            Method::DELETE => self.delete.0.load(ordering),
            Method::GET => self.get.0.load(ordering),
            Method::HEAD => self.head.0.load(ordering),
            Method::POST => self.post.0.load(ordering),
            Method::PUT => self.put.0.load(ordering),
            _ => 0,
        }
    }

    /// Reset all counters, following the provided memory ordering constrains
    ///
    /// Returns the respective counter values before reset.
    pub fn reset(&self, ordering: Ordering) -> RequestCounterValues {
        RequestCounterValues {
            delete: self.delete.0.swap(0, ordering),
            get: self.get.0.swap(0, ordering),
            head: self.head.0.swap(0, ordering),
            post: self.post.0.swap(0, ordering),
            put: self.put.0.swap(0, ordering),
            upload: self.upload.0.swap(0, ordering),
            download: self.download.0.swap(0, ordering),
        }
    }

    /// Account for new upload traffic.
    ///
    /// Returns the previously stored value.
    pub fn add_upload_traffic(&self, count: u64, ordering: Ordering) -> u64 {
        self.upload.0.fetch_add(count, ordering)
    }

    /// Returns upload traffic count.
    pub fn get_upload_traffic(&self, ordering: Ordering) -> u64 {
        self.upload.0.load(ordering)
    }

    /// Account for new download traffic.
    ///
    /// Returns the previously stored value.
    pub fn add_download_traffic(&self, count: u64, ordering: Ordering) -> u64 {
        self.download.0.fetch_add(count, ordering)
    }

    /// Returns download traffic count.
    pub fn get_download_traffic(&self, ordering: Ordering) -> u64 {
        self.download.0.load(ordering)
    }
}

/// Size of the padding to align the mmapped request counters to 4k default
/// page size.
const PADDING_SIZE: usize =
    MEMORY_PAGE_SIZE - std::mem::size_of::<AlignedMagic>() - std::mem::size_of::<RequestCounters>();

#[repr(C)]
// Alignment is chosen to reduce cache line contention while keeping low
// memory footprint.
struct MappableRequestCounters {
    magic: AlignedMagic,
    counters: RequestCounters,
    _page_size_padding: [u8; PADDING_SIZE],
}

impl Default for MappableRequestCounters {
    fn default() -> Self {
        Self {
            magic: AlignedMagic(PROXMOX_SHARED_REQUEST_COUNTERS_1_0),
            counters: RequestCounters::default(),
            _page_size_padding: [0; PADDING_SIZE],
        }
    }
}

impl Init for MappableRequestCounters {
    fn initialize(this: &mut MaybeUninit<Self>) {
        // safety: MappableRequestCounters contains simple data types with no internal references.
        this.write(MappableRequestCounters::default());
    }

    fn check_type_magic(this: &MaybeUninit<Self>) -> Result<(), Error> {
        unsafe {
            // safety: do not make assumptions about the object being initialized,
            // use raw pointer offsets to check memory for expected contents.
            let this_ptr = this.as_ptr();

            let magic_ptr = std::ptr::addr_of!((*this_ptr).magic);
            if *magic_ptr != AlignedMagic(PROXMOX_SHARED_REQUEST_COUNTERS_1_0) {
                bail!("incorrect magic number for request counters detected");
            }

            let counters_ptr = std::ptr::addr_of!((*this_ptr).counters);
            proxmox_shared_memory::check_subtype(&*counters_ptr)?;
        }
        Ok(())
    }
}

/// Atomic counters storing per-request method counts for the client.
///
/// If set, the counts can be filtered based on a path prefix.
pub struct SharedRequestCounters {
    shared_memory: SharedMemory<MappableRequestCounters>,
    path: PathBuf,
}

impl SharedRequestCounters {
    /// Create a new shared counter instance.
    ///
    /// Opens or creates mmap file and accesses it via shared memory mapping.
    pub fn open_shared_memory_mapped<P: AsRef<Path>>(path: P, user: User) -> Result<Self, Error> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            let dir_opts = CreateOptions::new()
                .perm(Mode::from_bits_truncate(0o770))
                .owner(user.uid)
                .group(user.gid);

            proxmox_sys::fs::create_path(parent, Some(dir_opts), Some(dir_opts))?;
        }

        let file_opts = CreateOptions::new()
            .perm(Mode::from_bits_truncate(0o660))
            .owner(user.uid)
            .group(user.gid);
        let shared_memory = SharedMemory::open_non_tmpfs(&path, file_opts)?;
        Ok(Self {
            shared_memory,
            path,
        })
    }

    /// Increment the counter for given method, following the provided memory ordering constrains
    ///
    /// Returns the previously stored value.
    pub fn increment(&self, method: Method, ordering: Ordering) -> u64 {
        self.shared_memory
            .data()
            .counters
            .increment(method, ordering)
    }

    /// Load current counter state for given method, following the provided memory ordering constrains
    pub fn load(&self, method: Method, ordering: Ordering) -> u64 {
        self.shared_memory.data().counters.load(method, ordering)
    }

    /// Reset all counters, following the provided memory ordering constrains
    ///
    /// Returns the respective counter values before reset.
    pub fn reset(&self, ordering: Ordering) -> RequestCounterValues {
        self.shared_memory.data().counters.reset(ordering)
    }

    /// Account for new upload traffic.
    ///
    /// Returns the previously stored value.
    pub fn add_upload_traffic(&self, count: u64, ordering: Ordering) -> u64 {
        self.shared_memory
            .data()
            .counters
            .add_upload_traffic(count, ordering)
    }

    /// Returns upload traffic count.
    pub fn get_upload_traffic(&self, ordering: Ordering) -> u64 {
        self.shared_memory
            .data()
            .counters
            .get_upload_traffic(ordering)
    }

    /// Account for new download traffic.
    ///
    /// Returns the previously stored value.
    pub fn add_download_traffic(&self, count: u64, ordering: Ordering) -> u64 {
        self.shared_memory
            .data()
            .counters
            .add_download_traffic(count, ordering)
    }

    /// Returns download traffic count.
    pub fn get_download_traffic(&self, ordering: Ordering) -> u64 {
        self.shared_memory
            .data()
            .counters
            .get_download_traffic(ordering)
    }

    /// Flush in-memory contents to backing file, but do not wait for completion
    pub fn schedule_flush(&self) -> Result<(), Error> {
        self.shared_memory.msync(MsFlags::MS_ASYNC)
    }

    /// Path of shared memory backing file
    pub fn path_buf(&self) -> PathBuf {
        self.path.clone()
    }
}

const FLUSH_THRESHOLD: Duration = Duration::from_secs(5);

// state for periodic flushing of the mmapped request counter values to the
// backend
pub(crate) struct MmapFlusher {
    task_handler: Option<TaskHandler>,
    register: Arc<RwLock<HashMap<PathBuf, CounterRegisterItem>>>,
}

struct CounterRegisterItem {
    register_count: usize,
    counters: Arc<SharedRequestCounters>,
}

struct TaskHandler {
    request_sender: mpsc::Sender<()>,
    task_handle: JoinHandle<()>,
    // Keep reference to runtime while task is being executed
    _runtime: Arc<tokio::runtime::Runtime>,
}

impl Drop for TaskHandler {
    fn drop(&mut self) {
        self.task_handle.abort();
    }
}

impl MmapFlusher {
    /// Create new empty and inactive flusher instance. Handler task will be created on-demand
    /// when the first counter is registered.
    pub(crate) fn new() -> Self {
        Self {
            task_handler: None,
            register: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register the shared request counter to be flushed periodically.
    pub(crate) fn register_counter(&mut self, counters: Arc<SharedRequestCounters>) {
        let id = counters.path_buf();

        if self.task_handler.is_none() {
            self.task_handler = Some(self.init_channel_and_task());
        }

        let mut register = self.register.write().unwrap();
        register
            .entry(id)
            .and_modify(|item| item.register_count += 1)
            .or_insert(CounterRegisterItem {
                register_count: 1,
                counters,
            });
    }

    /// Remove the shared request counter to no longer be flushed by the handler task.
    pub(crate) fn remove_counter(&mut self, id: &PathBuf) {
        let mut register = self.register.write().unwrap();
        if let Some(item) = register.remove(id) {
            if item.register_count > 1 {
                register.insert(
                    item.counters.path_buf(),
                    CounterRegisterItem {
                        register_count: item.register_count - 1,
                        counters: item.counters,
                    },
                );
            }
        }
        if register.is_empty() {
            // no more registered counters, abort task by dropping
            self.task_handler.take();
        }
    }

    /// Request for the flusher to be executed the next time the timeout is reached.
    pub(crate) fn request_flush(&self) -> Result<(), Error> {
        match self.task_handler.as_ref() {
            Some(handler) => {
                // ignore when channel full, flush already requested anyways
                if let Err(TrySendError::Closed(())) = handler.request_sender.try_send(()) {
                    bail!("failed to send flush request, channel closed");
                }
            }
            None => bail!("failed to send flush request, no task handler"),
        }
        Ok(())
    }

    /// Setup or get the current tokio runtime, create channel for requesting flushes and setup
    /// the task to periodically check for flush requests.
    fn init_channel_and_task(&self) -> TaskHandler {
        let (request_sender, mut request_receiver) = mpsc::channel(1);

        let register = Arc::clone(&self.register);
        let _runtime = proxmox_async::runtime::get_runtime();
        let task_handle = _runtime.spawn(async move {
            let mut flush_requested = false;
            let mut next_timeout = Instant::now() + FLUSH_THRESHOLD;

            loop {
                match tokio::time::timeout_at(next_timeout, request_receiver.recv()).await {
                    Ok(Some(())) => flush_requested = true,
                    Err(_timeout) => {
                        if flush_requested {
                            Self::handle_flush(Arc::clone(&register));
                            flush_requested = false;
                        }
                        next_timeout = Instant::now() + FLUSH_THRESHOLD;
                    }
                    _ => {
                        // channel closed or error
                        Self::handle_flush(Arc::clone(&register));
                        return;
                    }
                }
            }
        });

        TaskHandler {
            request_sender,
            task_handle,
            _runtime,
        }
    }

    // Helper to flush all currently registered shared request counters.
    fn handle_flush(register: Arc<RwLock<HashMap<PathBuf, CounterRegisterItem>>>) {
        let register = register.read().unwrap();
        for item in register.values() {
            if let Err(err) = item.counters.schedule_flush() {
                tracing::error!("failed to schedule flush: {err}");
            }
        }
    }
}

/// Current value of the individual request counters.
///
/// The values for the different fields are not guaranteed to be synchronized.
pub struct RequestCounterValues {
    /// number of GET requests
    pub get: u64,
    /// number of DELETE requests
    pub delete: u64,
    /// number of PUT requests
    pub put: u64,
    /// number of HEAD requests
    pub head: u64,
    /// number of POST requests
    pub post: u64,
    /// bytes uploaded
    pub upload: u64,
    /// bytes downloaded
    pub download: u64,
}
