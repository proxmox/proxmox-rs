//! A thread pool which run a closure in parallel.

use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crossbeam_channel::{bounded, Sender};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("send failed - channel closed")]
    ChannelClosed,

    #[error("handler failed: {0}")]
    HandlerFailed(String),

    #[error("thread {name} panicked")]
    ThreadPanicked {
        /// The name of the thread.
        name: String,
        /// The panic message extracted from the panic payload.
        message: Option<String>,
    },
}

/// A handle to send data to the worker thread (implements clone)
pub struct SendHandle<I> {
    input: Sender<I>,
    abort: Arc<Mutex<Option<String>>>,
}

/// Returns the first error happened, if any
fn check_abort(abort: &Mutex<Option<String>>) -> Result<(), Error> {
    let guard = abort.lock().unwrap();
    if let Some(err_msg) = &*guard {
        return Err(Error::HandlerFailed(err_msg.clone()));
    }
    Ok(())
}

impl<I: Send> SendHandle<I> {
    /// Send data to the worker threads
    pub fn send(&self, input: I) -> Result<(), Error> {
        check_abort(&self.abort)?;
        self.input.send(input).map_err(|_| Error::ChannelClosed)
    }
}

/// A thread pool which run the supplied closure
///
/// The send command sends data to the worker threads. If one handler
/// returns an error, we mark the channel as failed and it is no
/// longer possible to send data.
///
/// When done, the 'complete()' method needs to be called to check for
/// outstanding errors.
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
    /// Create a new thread pool, each thread processing incoming data
    /// with 'handler_fn'.
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

    /// Returns a cloneable channel to send data to the worker threads
    pub fn channel(&self) -> SendHandle<I> {
        self.input.as_ref().unwrap().clone()
    }

    /// Send data to the worker threads
    pub fn send(&self, input: I) -> Result<(), Error> {
        self.input.as_ref().unwrap().send(input)?;
        Ok(())
    }

    /// Wait for worker threads to complete and check for errors
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

// Note: We make sure that all threads will be joined
impl<I> Drop for ParallelHandler<I> {
    fn drop(&mut self) {
        drop(self.input.take());
        while let Some(handle) = self.handles.pop() {
            let _ = handle.join();
        }
    }
}
