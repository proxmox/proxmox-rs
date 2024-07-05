use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::Error;
use bytes::Bytes;
use flate2::{Decompress, FlushDecompress};
use futures::ready;
use futures::stream::Stream;

use proxmox_io::ByteBuffer;

#[derive(Eq, PartialEq)]
enum DecoderState {
    Reading,
    Writing,
    Flushing,
    Finished,
}

pub struct DeflateDecoder<T> {
    inner: T,
    decompressor: Decompress,
    buffer: ByteBuffer,
    input_buffer: Bytes,
    state: DecoderState,
}

pub struct DeflateDecoderBuilder<T> {
    inner: T,
    is_zlib: bool,
    buffer_size: usize,
}

impl<T> DeflateDecoderBuilder<T> {
    pub fn zlib(mut self, is_zlib: bool) -> Self {
        self.is_zlib = is_zlib;
        self
    }

    pub fn buffer_size(mut self, buffer_size: usize) -> Self {
        self.buffer_size = buffer_size;
        self
    }

    pub fn build(self) -> DeflateDecoder<T> {
        DeflateDecoder {
            inner: self.inner,
            decompressor: Decompress::new(self.is_zlib),
            buffer: ByteBuffer::with_capacity(self.buffer_size),
            input_buffer: Bytes::new(),
            state: DecoderState::Reading,
        }
    }
}

impl<T> DeflateDecoder<T> {
    pub fn new(inner: T) -> Self {
        Self::builder(inner).build()
    }

    pub fn builder(inner: T) -> DeflateDecoderBuilder<T> {
        DeflateDecoderBuilder {
            inner,
            is_zlib: false,
            buffer_size: super::BUFFER_SIZE,
        }
    }

    fn decode(
        &mut self,
        inbuf: &[u8],
        flush: FlushDecompress,
    ) -> Result<(usize, flate2::Status), io::Error> {
        let old_in = self.decompressor.total_in();
        let old_out = self.decompressor.total_out();
        let res = self
            .decompressor
            .decompress(inbuf, self.buffer.get_free_mut_slice(), flush)?;
        let new_in = (self.decompressor.total_in() - old_in) as usize;
        let new_out = (self.decompressor.total_out() - old_out) as usize;
        self.buffer.add_size(new_out);

        Ok((new_in, res))
    }
}

impl<T, O, E> Stream for DeflateDecoder<T>
where
    T: Stream<Item = Result<O, E>> + Unpin,
    O: Into<Bytes>,
    E: Into<Error>,
{
    type Item = Result<Bytes, anyhow::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            match this.state {
                DecoderState::Reading => {
                    if let Some(res) = ready!(Pin::new(&mut this.inner).poll_next(cx)) {
                        let buf = res.map_err(Into::into)?;
                        this.input_buffer = buf.into();
                        this.state = DecoderState::Writing;
                    } else {
                        this.state = DecoderState::Flushing;
                    }
                }
                DecoderState::Writing => {
                    if this.input_buffer.is_empty() {
                        return Poll::Ready(Some(Err(anyhow::format_err!(
                            "empty input during write"
                        ))));
                    }
                    let mut buf = this.input_buffer.split_off(0);
                    let (read, res) = this.decode(&buf[..], FlushDecompress::None)?;
                    this.input_buffer = buf.split_off(read);
                    if this.input_buffer.is_empty() {
                        this.state = DecoderState::Reading;
                    }
                    if this.buffer.is_full() || res == flate2::Status::BufError {
                        let bytes = this.buffer.remove_data(this.buffer.len()).to_vec();
                        return Poll::Ready(Some(Ok(bytes.into())));
                    }
                }
                DecoderState::Flushing => {
                    let (_read, res) = this.decode(&[][..], FlushDecompress::Finish)?;
                    if !this.buffer.is_empty() {
                        let bytes = this.buffer.remove_data(this.buffer.len()).to_vec();
                        return Poll::Ready(Some(Ok(bytes.into())));
                    }
                    if res == flate2::Status::StreamEnd {
                        this.state = DecoderState::Finished;
                    }
                }
                DecoderState::Finished => return Poll::Ready(None),
            }
        }
    }
}
