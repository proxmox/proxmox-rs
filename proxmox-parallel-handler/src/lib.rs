//! A thread pool that runs a closure in parallel across multiple worker threads.
//!
//! This crate provides [`ParallelHandler`], a simple thread pool that distributes work items of
//! type `I` to a fixed number of worker threads, each executing the same handler closure. Work is
//! submitted through a bounded [`crossbeam_channel`].
//!
//! If any worker's handler returns an error, the pool is marked as failed and subsequent
//! [`send`](ParallelHandler::send) calls will return the first recorded error. After all items
//! have been submitted, call [`complete`](ParallelHandler::complete) to join the worker threads
//! and surface any errors (including thread panics).
//!
//! # Example
//!
//! ```
//! use proxmox_parallel_handler::ParallelHandler;
//!
//! let pool = ParallelHandler::new("example", 4, |value: u64| {
//!     println!("processing {value}");
//!     Ok(())
//! });
//!
//! for i in 0..100 {
//!     pool.send(i)?;
//! }
//!
//! pool.complete()?;
//! # Ok::<(), proxmox_parallel_handler::Error>(())
//! ```

use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crossbeam_channel::{bounded, Sender};

/// Errors returned by [`ParallelHandler`] and [`SendHandle`] operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The internal channel has been closed.
    ///
    /// This typically means the worker threads have already shut down, either because
    /// [`ParallelHandler::complete`] was called or the pool was dropped.
    #[error("send failed - channel closed")]
    ChannelClosed,

    /// A worker thread's handler closure returned an error.
    ///
    /// Contains the formatted error message from the first handler that failed.
    /// Once a handler fails, all subsequent [`send`](SendHandle::send) calls will
    /// this error.
    #[error("handler failed: {0}")]
    HandlerFailed(String),

    /// A worker thread panicked.
    #[error("thread {name} panicked")]
    ThreadPanicked {
        /// The name of the thread.
        name: String,
        /// The panic message extracted from the panic payload.
        message: Option<String>,
    },
}

/// A cloneable handle for sending work items to a [`ParallelHandler`]'s worker threads.
///
/// Obtained via [`ParallelHandler::channel`]. Multiple clones of the same `SendHandle` share the
/// underlying channel and abort state, so they can be used from different threads or tasks to
/// submit work concurrently.
pub struct SendHandle<I> {
    input: Sender<I>,
    abort: Arc<Mutex<Option<String>>>,
}

/// Returns the first error which happened, if any.
fn check_abort(abort: &Mutex<Option<String>>) -> Result<(), Error> {
    let guard = abort.lock().unwrap();
    if let Some(err_msg) = &*guard {
        return Err(Error::HandlerFailed(err_msg.clone()));
    }
    Ok(())
}

impl<I: Send> SendHandle<I> {
    /// Send a work item to the worker threads.
    ///
    /// The item is placed into the bounded channel and will be picked up by the next idle
    /// worker. If all workers are busy, this call blocks until a worker becomes available.
    ///
    /// # Errors
    ///
    ///  - [`Error::HandlerFailed`] if any worker has already returned an error
    ///  - [`Error::ChannelClosed`] if the channel has been closed (e.g. the pool was dropped).
    pub fn send(&self, input: I) -> Result<(), Error> {
        check_abort(&self.abort)?;
        self.input.send(input).map_err(|_| Error::ChannelClosed)
    }
}

/// A thread pool that runs the supplied closure on each work item in parallel.
///
/// `ParallelHandler` spawns a fixed number of worker threads at construction time. Each thread
/// receives work items of type `I` through a shared bounded channel and processes them with a
/// cloned copy of the handler closure.
///
/// # Error handling
///
/// If any handler invocation returns an error, the pool records the first error message and
/// enters a failed state. Subsequent [`send`](Self::send) calls will immediately return
/// [`Error::HandlerFailed`] rather than enqueueing more work.
///
/// If the `ParallelHandler` is dropped without calling `complete`, the [`Drop`] implementation
/// still joins all threads, but any errors are silently discarded.
pub struct ParallelHandler<I> {
    handles: Vec<JoinHandle<()>>,
    input: Option<SendHandle<I>>,
}

impl<I> Clone for SendHandle<I> {
    fn clone(&self) -> Self {
        Self {
            input: self.input.clone(),
            abort: Arc::clone(&self.abort),
        }
    }
}

impl<I: Send + 'static> ParallelHandler<I> {
    /// Create a new thread pool with `threads` workers, each processing incoming data with
    /// `handler_fn`.
    ///
    /// # Parameters
    ///
    /// - `name` - A human-readable name used in thread names and error messages.
    /// - `threads` - The number of worker threads to spawn.
    /// - `handler_fn` - The closure invoked for every work item.
    pub fn new<F>(name: &str, threads: usize, handler_fn: F) -> Self
    where
        F: Fn(I) -> Result<(), anyhow::Error> + Send + Clone + 'static,
    {
        let mut handles = Vec::new();
        let (input_tx, input_rx) = bounded::<I>(threads);

        let abort = Arc::new(Mutex::new(None));

        for i in 0..threads {
            let input_rx = input_rx.clone();
            let abort = Arc::clone(&abort);
            let handler_fn = handler_fn.clone();

            handles.push(
                std::thread::Builder::new()
                    .name(format!("{name} ({i})"))
                    .spawn(move || loop {
                        let data = match input_rx.recv() {
                            Ok(data) => data,
                            Err(_) => return,
                        };
                        if let Err(err) = (handler_fn)(data) {
                            let mut guard = abort.lock().unwrap();
                            if guard.is_none() {
                                *guard = Some(format!("{err:#}"));
                            }
                        }
                    })
                    // unwrap is fine, `spawn` only panics if a thread name with null bytes as
                    // set
                    .unwrap(),
            );
        }
        Self {
            handles,
            input: Some(SendHandle {
                input: input_tx,
                abort,
            }),
        }
    }

    /// Returns a cloneable [`SendHandle`] that can be used to send work items to the worker
    /// threads.
    ///
    /// This is useful when you need to send items from multiple threads or tasks concurrently.
    /// Each clone of the returned handle shares the same underlying channel.
    pub fn channel(&self) -> SendHandle<I> {
        // unwrap: fine as long as Self::complete has not been called yet. Since
        // Self::complete takes self, this cannot happen for any of our callers.
        self.input.as_ref().unwrap().clone()
    }

    /// Send a work item to the worker threads.
    ///
    /// Convenience wrapper around the internal [`SendHandle::send`]. Blocks if the bounded
    /// channel is full (i.e. all workers are busy).
    ///
    /// # Errors
    ///
    ///  - [`Error::HandlerFailed`] if any worker has already returned an error
    ///  - [`Error::ChannelClosed`] if the channel has been closed.
    pub fn send(&self, input: I) -> Result<(), Error> {
        // unwrap: fine as long as Self::complete has not been called yet. Since
        // Self::complete takes self, this cannot happen for any of our callers.
        self.input.as_ref().unwrap().send(input)?;
        Ok(())
    }

    /// Close the channel, wait for all worker threads to finish, and check for errors.
    ///
    /// # Errors
    ///
    /// - [`Error::HandlerFailed`] - if any handler returned an error.
    /// - [`Error::ThreadPanicked`] - if a worker thread panicked.
    pub fn complete(mut self) -> Result<(), Error> {
        let input = self.input.take().unwrap();
        let abort = Arc::clone(&input.abort);
        check_abort(&abort)?;
        drop(input);

        let mut msg_list = self.join_threads();

        // an error might be encountered while waiting for the join
        check_abort(&abort)?;

        if let Some(e) = msg_list.pop() {
            // Any error here is due to a thread panicking - let's just report that
            // last panic that occurred.
            Err(e)
        } else {
            Ok(())
        }
    }

    fn join_threads(&mut self) -> Vec<Error> {
        let mut msg_list = Vec::new();

        while let Some(handle) = self.handles.pop() {
            let thread_name = handle.thread().name().unwrap_or("<unknown>").to_string();

            if let Err(panic) = handle.join() {
                if let Some(message) = panic.downcast_ref::<&str>() {
                    msg_list.push(Error::ThreadPanicked {
                        name: thread_name,
                        message: Some(message.to_string()),
                    });
                } else if let Some(message) = panic.downcast_ref::<String>() {
                    msg_list.push(Error::ThreadPanicked {
                        name: thread_name,
                        message: Some(message.to_string()),
                    });
                } else {
                    msg_list.push(Error::ThreadPanicked {
                        name: thread_name,
                        message: None,
                    });
                }
            }
        }
        msg_list
    }
}

/// Dropping a `ParallelHandler` closes the channel and joins all worker threads.
///
/// Any errors that occurred in handler closures or thread panics are silently discarded.
/// Prefer calling [`ParallelHandler::complete`] explicitly if you need to observe errors.
impl<I> Drop for ParallelHandler<I> {
    fn drop(&mut self) {
        drop(self.input.take());
        while let Some(handle) = self.handles.pop() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_send_on_pool() {
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&count);

        let pool = ParallelHandler::new("ok", 2, move |_: u32| {
            count_clone.fetch_add(1, Ordering::Relaxed);
            Ok(())
        });

        for i in 0..10 {
            pool.send(i).unwrap();
        }

        pool.complete().unwrap();
        assert_eq!(count.load(Ordering::Relaxed), 10);
    }

    #[test]
    fn test_send_on_handle() {
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&count);

        let pool = ParallelHandler::new("chan", 2, move |_: u32| {
            count_clone.fetch_add(1, Ordering::Relaxed);
            Ok(())
        });

        let handle = pool.channel();
        for i in 0..5 {
            handle.send(i).unwrap();
        }
        drop(handle);

        pool.complete().unwrap();
        assert_eq!(count.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn handler_error_is_propagated_on_complete() {
        let pool = ParallelHandler::new("fail", 1, |_: u32| {
            anyhow::bail!("boom");
        });

        pool.send(1).unwrap();
        let err = pool.complete().unwrap_err();

        match err {
            Error::HandlerFailed(msg) => assert!(msg.contains("boom")),
            _ => panic!("invalid error variant"),
        }
    }

    #[test]
    fn thread_panic_is_reported_on_complete() {
        let pool = ParallelHandler::new("panic", 1, |_: u32| -> Result<(), anyhow::Error> {
            panic!("boom");
        });

        pool.send(1).unwrap();
        let err = pool.complete().unwrap_err();
        match err {
            Error::ThreadPanicked { message, .. } => assert!(message.unwrap().contains("boom")),
            _ => panic!("invalid error variant"),
        }
    }
}
