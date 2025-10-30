use std::io::Write;
use std::string::ToString;
use std::sync::mpsc::SyncSender;

/// Wrapper around SyncSender, which implements Write
///
/// Each write in translated into a `send(Vec<u8>)` (that is, for each write, an owned byte vector
/// is allocated and sent over the channel).
pub struct StdChannelWriter<E>(SyncSender<Result<Vec<u8>, E>>);

impl<E: ToString> StdChannelWriter<E> {
    pub fn new(sender: SyncSender<Result<Vec<u8>, E>>) -> Self {
        Self(sender)
    }
}

impl<E: ToString> Write for StdChannelWriter<E> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.0
            .send(Ok(buf.to_vec()))
            .map_err(|err| std::io::Error::other(err.to_string()))
            .and(Ok(buf.len()))
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}
