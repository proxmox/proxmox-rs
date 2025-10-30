use std::io;

use anyhow::Error;
use tokio::sync::mpsc::Sender;

/// Wrapper struct around [`tokio::sync::mpsc::Sender`] for `Result<Vec<u8>, Error>` that implements [`std::io::Write`]
pub struct SenderWriter {
    sender: Sender<Result<Vec<u8>, Error>>,
}

impl SenderWriter {
    pub fn from_sender(sender: tokio::sync::mpsc::Sender<Result<Vec<u8>, Error>>) -> Self {
        Self { sender }
    }

    fn write_impl(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Err(err) = self.sender.blocking_send(Ok(buf.to_vec())) {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("could not send: {err}"),
            ));
        }

        Ok(buf.len())
    }

    fn flush_impl(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl io::Write for SenderWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_impl(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_impl()
    }
}

impl Drop for SenderWriter {
    fn drop(&mut self) {
        // ignore errors
        let _ = self.flush_impl();
    }
}
