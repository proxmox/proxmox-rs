use std::future::Future;
use std::pin::pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use anyhow::{bail, Error};
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::watch;

static SHUTDOWN_LISTENERS: OnceLock<watch::Sender<bool>> = OnceLock::new();
static RELOAD_REQUESTED: AtomicBool = AtomicBool::new(false);
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Request a reload.
///
/// This sets the reload flag and subsequently calls [`request_shutdown()`].
pub fn request_reload() {
    if !RELOAD_REQUESTED.swap(true, Ordering::Release) {
        request_shutdown();
    }
}

/// Returns true if a reload has been requested either via a signal or a call to
/// [`request_reload()`].
pub fn is_reload_requested() -> bool {
    RELOAD_REQUESTED.load(Ordering::Acquire)
}

/// Request a shutdown.
///
/// This sets both the shutdown flag and triggers [`shutdown_future()`] to finish.
pub fn request_shutdown() {
    log::info!("request_shutdown");

    if !SHUTDOWN_REQUESTED.swap(true, Ordering::Release) {
        let _ = shutdown_listeners().send(true);
    }
}

/// Returns true if a shutdown has been requested either via a signal or a call to
/// [`request_shutdown()`].
pub fn is_shutdown_requested() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::Acquire)
}

fn shutdown_listeners() -> &'static watch::Sender<bool> {
    SHUTDOWN_LISTENERS.get_or_init(|| watch::channel(false).0)
}

/// This future finishes once a shutdown has been requested either via a signal or a call to
/// [`request_shutdown()`].
pub async fn shutdown_future() {
    let _ = shutdown_listeners().subscribe().wait_for(|&v| v).await;
}

/// Pin and select().
async fn pin_select<A, B>(a: A, b: B)
where
    A: Future<Output = ()> + Send + 'static,
    B: Future<Output = ()> + Send + 'static,
{
    let a = pin!(a);
    let b = pin!(b);
    futures::future::select(a, b).await;
}

/// Creates a task which listens for a `SIGINT` and then calls [`request_shutdown()`] while also
/// *undoing* a previous *reload* request.
pub fn shutdown_signal_task() -> Result<impl Future<Output = ()> + Send + 'static, Error> {
    let mut stream = signal(SignalKind::interrupt())?;

    Ok(async move {
        while stream.recv().await.is_some() {
            log::info!("got shutdown request (SIGINT)");
            RELOAD_REQUESTED.store(false, Ordering::Release);
            request_shutdown();
        }
    })
}

/// Spawn a [`shutdown_signal_task()`] which is automatically aborted with the provided
/// `abort_future`.
pub fn catch_shutdown_signal<F>(abort_future: F) -> Result<(), Error>
where
    F: Future<Output = ()> + Send + 'static,
{
    log::info!("catching shutdown signal");
    tokio::spawn(pin_select(shutdown_signal_task()?, abort_future));
    Ok(())
}

/// Creates a task which listens for a `SIGHUP` and then calls [`request_reload()`].
pub fn reload_signal_task() -> Result<impl Future<Output = ()> + Send + 'static, Error> {
    let mut stream = signal(SignalKind::hangup())?;

    Ok(async move {
        while stream.recv().await.is_some() {
            log::info!("got reload request (SIGHUP)");
            request_reload();
        }
    })
}

/// Spawn a [`reload_signal_task()`] which is automatically aborted with the provided
/// `abort_future`.
pub fn catch_reload_signal<F>(abort_future: F) -> Result<(), Error>
where
    F: Future<Output = ()> + Send + 'static,
{
    log::info!("catching reload signal");
    tokio::spawn(pin_select(reload_signal_task()?, abort_future));
    Ok(())
}

/// Raise an error if there was a shutdown request.
pub fn fail_on_shutdown() -> Result<(), Error> {
    if is_shutdown_requested() {
        bail!("Server shutdown requested - aborting task");
    }
    Ok(())
}
