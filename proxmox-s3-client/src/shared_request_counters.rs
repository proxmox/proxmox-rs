use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, RwLock};
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

use crate::api_types::RequestCounterThresholds;

const MEMORY_PAGE_SIZE: usize = 4096;
/// Generated via openssl::sha::sha256(b"Proxmox shared request counters v1.0")[0..8]
const PROXMOX_SHARED_REQUEST_COUNTERS_1_0: [u8; 8] = [224, 110, 88, 252, 26, 77, 180, 5];

/// Callback method triggered when exceeding counter thresholds.
/// Callback is called with the following parameters: common-prefix, threshold name, threshold limit,
/// value exceeding limit.
pub type ThresholdExceededCallback = Box<dyn Fn(&str, &str, u64, u64) + Send + Sync + 'static>;
static SHARED_COUNTER_THRESHOLD_EXCEEDED_CALLBACK: LazyLock<
    RwLock<Option<ThresholdExceededCallback>>,
> = LazyLock::new(|| RwLock::new(None));

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

    /// Request counter thresholds
    get_threshold: AlignedAtomic,
    delete_threshold: AlignedAtomic,
    put_threshold: AlignedAtomic,
    head_threshold: AlignedAtomic,
    post_threshold: AlignedAtomic,
    // Traffic counter thresholds
    upload_threshold: AlignedAtomic,
    download_threshold: AlignedAtomic,
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
    pub fn increment(&self, cb_label: &str, method: Method, ordering: Ordering) -> u64 {
        match method {
            Method::DELETE => {
                let prev = self.delete.0.fetch_add(1, ordering);
                let threshold = self.delete_threshold.0.load(Ordering::Acquire);
                Self::check_threshold(cb_label, method.as_str(), threshold, prev + 1);
                prev
            }
            Method::GET => {
                let prev = self.get.0.fetch_add(1, ordering);
                let threshold = self.get_threshold.0.load(Ordering::Acquire);
                Self::check_threshold(cb_label, method.as_str(), threshold, prev + 1);
                prev
            }
            Method::HEAD => {
                let prev = self.head.0.fetch_add(1, ordering);
                let threshold = self.head_threshold.0.load(Ordering::Acquire);
                Self::check_threshold(cb_label, method.as_str(), threshold, prev + 1);
                prev
            }
            Method::POST => {
                let prev = self.post.0.fetch_add(1, ordering);
                let threshold = self.post_threshold.0.load(Ordering::Acquire);
                Self::check_threshold(cb_label, method.as_str(), threshold, prev + 1);
                prev
            }
            Method::PUT => {
                let prev = self.put.0.fetch_add(1, ordering);
                let threshold = self.put_threshold.0.load(Ordering::Acquire);
                Self::check_threshold(cb_label, method.as_str(), threshold, prev + 1);
                prev
            }
            _ => 0,
        }
    }

    fn check_threshold(cb_label: &str, counter_id: &str, threshold: u64, current: u64) {
        if threshold > 0 && current > threshold && current - 1 == threshold {
            let guard = SHARED_COUNTER_THRESHOLD_EXCEEDED_CALLBACK.read().unwrap();
            if let Some(callback) = guard.as_ref() {
                callback(cb_label, counter_id, threshold, current);
            }
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
    pub fn add_upload_traffic(&self, cb_label: &str, count: u64, ordering: Ordering) -> u64 {
        let prev = self.upload.0.fetch_add(count, ordering);
        let threshold = self.upload_threshold.0.load(Ordering::Acquire);
        let uploaded = prev + count;
        if threshold > 0 && uploaded > threshold && prev <= threshold {
            let guard = SHARED_COUNTER_THRESHOLD_EXCEEDED_CALLBACK.read().unwrap();
            if let Some(callback) = guard.as_ref() {
                callback(cb_label, "uploaded", threshold, uploaded);
            }
        }
        prev
    }

    /// Returns upload traffic count.
    pub fn get_upload_traffic(&self, ordering: Ordering) -> u64 {
        self.upload.0.load(ordering)
    }

    /// Account for new download traffic.
    ///
    /// Returns the previously stored value.
    pub fn add_download_traffic(&self, cb_label: &str, count: u64, ordering: Ordering) -> u64 {
        let prev = self.download.0.fetch_add(count, ordering);
        let threshold = self.download_threshold.0.load(Ordering::Acquire);
        let downloaded = prev + count;
        if threshold > 0 && downloaded > threshold && prev <= threshold {
            let guard = SHARED_COUNTER_THRESHOLD_EXCEEDED_CALLBACK.read().unwrap();
            if let Some(callback) = guard.as_ref() {
                callback(cb_label, "downloaded", threshold, downloaded);
            }
        }
        prev
    }

    /// Returns download traffic count.
    pub fn get_download_traffic(&self, ordering: Ordering) -> u64 {
        self.download.0.load(ordering)
    }

    /// Update the request threshold values.
    pub fn update_thresholds(&self, thresholds: &RequestCounterThresholds) {
        self.delete_threshold
            .0
            .store(thresholds.s3_delete.unwrap_or(0), Ordering::Release);
        self.get_threshold
            .0
            .store(thresholds.s3_get.unwrap_or(0), Ordering::Release);
        self.head_threshold
            .0
            .store(thresholds.s3_head.unwrap_or(0), Ordering::Release);
        self.post_threshold
            .0
            .store(thresholds.s3_post.unwrap_or(0), Ordering::Release);
        self.put_threshold
            .0
            .store(thresholds.s3_put.unwrap_or(0), Ordering::Release);
        let download = thresholds
            .s3_download
            .map(|human_byte| human_byte.as_u64())
            .unwrap_or(0);
        self.download_threshold.0.store(download, Ordering::Release);
        let upload = thresholds
            .s3_upload
            .map(|human_byte| human_byte.as_u64())
            .unwrap_or(0);
        self.upload_threshold.0.store(upload, Ordering::Release);
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
    cb_label: String,
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
            cb_label: String::new(),
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
            .increment(&self.cb_label, method, ordering)
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
            .add_upload_traffic(&self.cb_label, count, ordering)
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
            .add_download_traffic(&self.cb_label, count, ordering)
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

    /// Persist in-memory contents to backing file, blocking until synced
    pub fn flush(&self) -> Result<(), Error> {
        self.shared_memory.msync(MsFlags::MS_SYNC)
    }

    /// Path of shared memory backing file
    pub fn path_buf(&self) -> PathBuf {
        self.path.clone()
    }

    /// Update the callback and the label to identify the counter executing it
    /// when one of the set thresholds is exceeded.
    pub fn set_thresholds_exceeded_callback(
        &mut self,
        cb_label: String,
        callback: ThresholdExceededCallback,
    ) {
        self.cb_label = cb_label;
        SHARED_COUNTER_THRESHOLD_EXCEEDED_CALLBACK
            .write()
            .unwrap()
            .replace(callback);
    }

    /// Update the request counter thresholds to given values.
    pub fn update_thresholds(&self, thresholds: &RequestCounterThresholds) {
        self.shared_memory
            .data()
            .counters
            .update_thresholds(thresholds);
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
