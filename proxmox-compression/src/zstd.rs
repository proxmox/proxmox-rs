//! zstd helper
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::{format_err, Error};
use bytes::Bytes;
use futures::ready;
use futures::stream::Stream;
use zstd::stream::raw::{Encoder, Operation, OutBuffer};

use proxmox_io::ByteBuffer;

const BUFFER_SIZE: usize = 8192;

#[derive(Eq, PartialEq)]
enum EncoderState {
    Reading,
    Writing,
    Finishing,
    Finished,
}

/// An async ZstdEncoder that implements [Stream] for another [Stream]
///
/// Useful for on-the-fly zstd compression in streaming api calls
pub struct ZstdEncoder<'a, T> {
    inner: T,
    compressor: Encoder<'a>,
    buffer: ByteBuffer,
    input_buffer: Bytes,
    state: EncoderState,
}

impl<T, O, E> ZstdEncoder<'_, T>
where
    T: Stream<Item = Result<O, E>> + Unpin,
    O: Into<Bytes>,
    E: Into<Error>,
{
    /// Returns a new [ZstdEncoder] with default level 3
    pub fn new(inner: T) -> Result<Self, io::Error> {
        Self::with_quality(inner, 3)
    }

    /// Returns a new [ZstdEncoder] with the given level
    pub fn with_quality(inner: T, level: i32) -> Result<Self, io::Error> {
        Ok(Self {
            inner,
            compressor: Encoder::new(level)?,
            buffer: ByteBuffer::with_capacity(BUFFER_SIZE),
            input_buffer: Bytes::new(),
            state: EncoderState::Reading,
        })
    }
}

impl<T> ZstdEncoder<'_, T> {
    /// Returns the wrapped [Stream]
    pub fn into_inner(self) -> T {
        self.inner
    }

    fn encode(&mut self, inbuf: &[u8]) -> Result<zstd::stream::raw::Status, io::Error> {
        let res = self
            .compressor
            .run_on_buffers(inbuf, self.buffer.get_free_mut_slice())?;
        self.buffer.add_size(res.bytes_written);

        Ok(res)
    }

    fn finish(&mut self) -> Result<usize, io::Error> {
        let mut outbuf = OutBuffer::around(self.buffer.get_free_mut_slice());
        let res = self.compressor.finish(&mut outbuf, true);
        let size = outbuf.pos();
        // drop(outbuf);
        self.buffer.add_size(size);
        res
    }
}

impl<T, O, E> Stream for ZstdEncoder<'_, T>
where
    T: Stream<Item = Result<O, E>> + Unpin,
    O: Into<Bytes>,
    E: Into<Error>,
{
    type Item = Result<Bytes, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            match this.state {
                EncoderState::Reading => {
                    if let Some(res) = ready!(Pin::new(&mut this.inner).poll_next(cx)) {
                        let buf = res.map_err(Into::into)?;
                        this.input_buffer = buf.into();
                        this.state = EncoderState::Writing;
                    } else {
                        this.state = EncoderState::Finishing;
                    }
                }
                EncoderState::Writing => {
                    if this.input_buffer.is_empty() {
                        return Poll::Ready(Some(Err(format_err!("empty input during write"))));
                    }
                    let mut buf = this.input_buffer.split_off(0);
                    let status = this.encode(&buf[..])?;
                    this.input_buffer = buf.split_off(status.bytes_read);
                    if this.input_buffer.is_empty() {
                        this.state = EncoderState::Reading;
                    }
                    if this.buffer.is_full() {
                        let bytes = this.buffer.remove_data(this.buffer.len()).to_vec();
                        return Poll::Ready(Some(Ok(bytes.into())));
                    }
                }
                EncoderState::Finishing => {
                    let remaining = this.finish()?;
                    if remaining == 0 {
                        this.state = EncoderState::Finished;
                    }
                    if !this.buffer.is_empty() {
                        let bytes = this.buffer.remove_data(this.buffer.len()).to_vec();
                        return Poll::Ready(Some(Ok(bytes.into())));
                    }
                }
                EncoderState::Finished => return Poll::Ready(None),
            }
        }
    }
}
