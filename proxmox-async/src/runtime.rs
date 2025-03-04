//! Helpers for quirks of the current tokio runtime.
//!
//! It is preferred to use these helpers throughout our applications.
//!
//! # `tokio`, Runtime Flavors, and Panics
//!
//! Because [`tokio`] may introduce more [`RuntimeFlavor`s][RuntimeFlavor] in the future,
//! we [`panic!`] on flavors we're not (yet) explicitly supporting.
//!
//! This is done for forward-compatibility's sake in order to prevent unforeseen
//! interactions with [`tokio`], such as with [`tokio::task::block_in_place`],
//! which [`panic!`s][panic!] *only* if called within a [`CurrentThread`][ct-rt]-flavored
//! runtime, but not in a [`MultiThread`][mt-rt]-flavored runtime or if there's
//! *no runtime* at all.
//!
//! All [`panic!`s][panic!] can otherwise be either avoided or caught early by instantiating
//! your runtime with [`get_runtime()`] or [`get_runtime_with_builder()`]. Or, if you're
//! creating a separate async application, use [`main()`] for convenience.
//!
//! ## Supported [`RuntimeFlavor`s][RuntimeFlavor]
//!
//! * [`RuntimeFlavor::MultiThread`]
//! * [`RuntimeFlavor::CurrentThread`]
//!
//! # [`tokio`] and OpenSSL
//!
//! There's a nasty [OpenSSL bug][openssl-bug] causing a race between OpenSSL clean-up handlers
//! and the [`tokio`] runtime. This however is handled by [`get_runtime_with_builder()`]
//! and thus also within [`get_runtime()`] and our [`main()`] wrapper.
//!
//! [ct-rt]: RuntimeFlavor::CurrentThread
//! [mt-rt]: RuntimeFlavor::MultiThread
//! [openssl-bug]: https://github.com/openssl/openssl/issues/6214

use std::future::Future;
use std::sync::{Arc, LazyLock, Mutex, Weak};
use std::task::{Context, Poll, Waker};
use std::thread::{self, Thread};

use pin_utils::pin_mut;
use tokio::runtime::{self, Runtime, RuntimeFlavor};

// avoid openssl bug: https://github.com/openssl/openssl/issues/6214
// by dropping the runtime as early as possible
static RUNTIME: LazyLock<Mutex<Weak<Runtime>>> = LazyLock::new(|| Mutex::new(Weak::new()));

#[link(name = "crypto")]
unsafe extern "C" {
    fn OPENSSL_thread_stop();
}

#[inline]
fn panic_on_bad_flavor(runtime: &runtime::Runtime) {
    match runtime.handle().runtime_flavor() {
        RuntimeFlavor::CurrentThread => (),
        RuntimeFlavor::MultiThread => (),
        bad_flavor => panic!("unsupported tokio runtime flavor: \"{:#?}\"", bad_flavor),
    }
}

/// Get or build the current main [`tokio`] [`Runtime`]. Useful if [`tokio`'s][tokio] defaults
/// don't suit your needs.
///
/// # Panics
/// This function will panic if the runtime has an unsupported [`RuntimeFlavor`].
/// See the [module level][mod] documentation for more details.
///
/// [mod]: self
pub fn get_runtime_with_builder<F: Fn() -> runtime::Builder>(get_builder: F) -> Arc<Runtime> {
    let mut guard = RUNTIME.lock().unwrap();

    if let Some(rt) = guard.upgrade() {
        panic_on_bad_flavor(&rt);
        return rt;
    }

    let mut builder = get_builder();
    builder.on_thread_stop(|| {
        // avoid openssl bug: https://github.com/openssl/openssl/issues/6214
        // call OPENSSL_thread_stop to avoid race with openssl cleanup handlers
        unsafe {
            OPENSSL_thread_stop();
        }
    });

    let runtime = builder.build().expect("failed to spawn tokio runtime");
    panic_on_bad_flavor(&runtime);

    let rt = Arc::new(runtime);

    *guard = Arc::downgrade(&rt);

    rt
}

/// Get or create the current main [`tokio`] [`Runtime`].
///
/// This is a convenience wrapper around [`get_runtime_with_builder()`] using
/// [`tokio`'s multithreaded runtime][mt-rt-meth].
///
/// [mt-rt-meth]: tokio::runtime::Builder::new_multi_thread()
pub fn get_runtime() -> Arc<Runtime> {
    get_runtime_with_builder(|| {
        let mut builder = runtime::Builder::new_multi_thread();
        builder.enable_all();
        builder
    })
}

/// Block on a synchronous piece of code.
///
/// This is a wrapper around [`tokio::task::block_in_place()`] that allows to
/// block the current thread even within a [`Runtime`] with [`RuntimeFlavor::CurrentThread`].
///
/// Normally, [tokio's `block_in_place()`][bip] [`panic`s][panic] when called in
/// such a case; this function instead just runs the piece of code right away, preventing
/// an unforeseen panic.
///
/// # Note
/// If you're in a [`CurrentThread`][RuntimeFlavor::CurrentThread] runtime and you
/// *really* need to execute a bunch of blocking code, you might want to consider
/// executing that code with [`tokio::task::spawn_blocking()`] instead. This prevents
/// blocking the single-threaded runtime and still allows you to communicate via channels.
///
/// See [tokio's documentation on CPU-bound tasks and blocking code][tok-block-doc]
/// for more information.
///
/// # Panics
/// This function will panic if the runtime has an unsupported [`RuntimeFlavor`].
/// See the [module level][mod] documentation for more details.
///
/// [bip]: tokio::task::block_in_place()
/// [mod]: self
/// [sp]: tokio::task::spawn_blocking()
/// [tok-block-doc]: https://docs.rs/tokio/latest/tokio/index.html#cpu-bound-tasks-and-blocking-code
pub fn block_in_place<R>(func: impl FnOnce() -> R) -> R {
    if let Ok(runtime) = runtime::Handle::try_current() {
        match runtime.runtime_flavor() {
            RuntimeFlavor::CurrentThread => func(),
            RuntimeFlavor::MultiThread => tokio::task::block_in_place(func),
            bad_flavor => panic!("unsupported tokio runtime flavor: \"{:#?}\"", bad_flavor),
        }
    } else {
        func()
    }
}

/// Block on a future in the current thread.
///
/// Not to be confused with [`tokio::runtime::Handle::block_on()`] and
/// [`tokio::runtime::Runtime::block_on()`].
///
/// This will prevent other futures from running in the current thread in the meantime.
/// Essentially, this is [`block_in_place()`], but for [`Future`s][Future] instead of functions.
///
/// If there's no runtime currently active, this function will create a temporary one
/// using [`get_runtime()`] in order to block on and finish running the provided [`Future`].
///
/// # Panics
/// This function will panic if the runtime has an unsupported [`RuntimeFlavor`].
/// See the [module level][mod] documentation for more details.
///
/// [mod]: self
pub fn block_on<F: Future>(future: F) -> F::Output {
    if let Ok(runtime) = runtime::Handle::try_current() {
        match runtime.runtime_flavor() {
            RuntimeFlavor::CurrentThread => block_on_local_future(future),
            RuntimeFlavor::MultiThread => {
                tokio::task::block_in_place(move || block_on_local_future(future))
            }
            bad_flavor => panic!("unsupported tokio runtime flavor: \"{:#?}\"", bad_flavor),
        }
    } else {
        let runtime = get_runtime();
        let _enter_guard = runtime.enter();

        runtime.block_on(future)
    }
}

/// This is our [`tokio`] entrypoint, which blocks on the provided [`Future`]
/// until it's completed, using [`tokio`'s multithreaded runtime][mt-rt-meth].
///
/// It is preferred to use this function over other ways of instantiating a runtime.
/// See the [module level][mod] documentation for more information.
///
/// [mod]: self
/// [mt-rt-meth]: tokio::runtime::Builder::new_multi_thread()
pub fn main<F: Future>(fut: F) -> F::Output {
    let runtime = get_runtime();
    let _enter_guard = runtime.enter();

    runtime.block_on(fut)
}

struct ThreadWaker(Thread);

impl std::task::Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on_local_future<F: Future>(fut: F) -> F::Output {
    pin_mut!(fut);

    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut context = Context::from_waker(&waker);
    loop {
        match fut.as_mut().poll(&mut context) {
            Poll::Ready(out) => return out,
            Poll::Pending => thread::park(),
        }
    }
}
