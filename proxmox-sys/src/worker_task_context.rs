use anyhow::{bail, Error};

/// Worker task abstraction
///
/// A worker task is a long running task, which usually logs output into a separate file.
pub trait WorkerTaskContext: Send + Sync {
    /// Test if there was a request to abort the task.
    fn abort_requested(&self) -> bool;

    /// If the task should be aborted, this should fail with a reasonable error message.
    fn check_abort(&self) -> Result<(), Error> {
        if self.abort_requested() {
            bail!("abort requested - aborting task");
        }
        Ok(())
    }

    /// Test if there was a request to shutdown the server.
    fn shutdown_requested(&self) -> bool;

    /// This should fail with a reasonable error message if there was
    /// a request to shutdown the server.
    fn fail_on_shutdown(&self) -> Result<(), Error> {
        if self.shutdown_requested() {
            bail!("Server shutdown requested - aborting task");
        }
        Ok(())
    }
}

/// Convenience implementation:
impl<T: WorkerTaskContext + ?Sized> WorkerTaskContext for std::sync::Arc<T> {
    fn abort_requested(&self) -> bool {
        <T as WorkerTaskContext>::abort_requested(self)
    }

    fn check_abort(&self) -> Result<(), Error> {
        <T as WorkerTaskContext>::check_abort(self)
    }

    fn shutdown_requested(&self) -> bool {
        <T as WorkerTaskContext>::shutdown_requested(self)
    }

    fn fail_on_shutdown(&self) -> Result<(), Error> {
        <T as WorkerTaskContext>::fail_on_shutdown(self)
    }
}
