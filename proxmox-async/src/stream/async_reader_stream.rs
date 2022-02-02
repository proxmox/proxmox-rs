//! Wrappers between async readers and streams.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::ready;
use futures::stream::Stream;
use tokio::io::{AsyncRead, ReadBuf};

/// Wrapper struct to convert an [AsyncRead] into a [Stream]
pub struct AsyncReaderStream<R: AsyncRead + Unpin> {
    reader: R,
    buffer: Vec<u8>,
}

impl<R: AsyncRead + Unpin> AsyncReaderStream<R> {
    pub fn new(reader: R) -> Self {
        let mut buffer = Vec::with_capacity(64 * 1024);
        unsafe {
            buffer.set_len(buffer.capacity());
        }
        Self { reader, buffer }
    }

    pub fn with_buffer_size(reader: R, buffer_size: usize) -> Self {
        let mut buffer = Vec::with_capacity(buffer_size);
        unsafe {
            buffer.set_len(buffer.capacity());
        }
        Self { reader, buffer }
    }
}

impl<R: AsyncRead + Unpin> Stream for AsyncReaderStream<R> {
    type Item = Result<Vec<u8>, io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let mut read_buf = ReadBuf::new(&mut this.buffer);
        match ready!(Pin::new(&mut this.reader).poll_read(cx, &mut read_buf)) {
            Ok(()) => {
                let n = read_buf.filled().len();
                if n == 0 {
                    // EOF
                    Poll::Ready(None)
                } else {
                    Poll::Ready(Some(Ok(this.buffer[..n].to_vec())))
                }
            }
            Err(err) => Poll::Ready(Some(Err(err))),
        }
    }
}
