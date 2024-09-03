use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::Error;
use bytes::Bytes;
use flate2::{Compress, Compression, FlushCompress};
use futures::ready;
use futures::stream::Stream;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use proxmox_io::ByteBuffer;
use proxmox_lang::io_format_err;

pub enum Level {
    Fastest,
    Best,
    Default,
    Precise(u32),
}

#[derive(Eq, PartialEq)]
enum EncoderState {
    Reading,
    Writing,
    Flushing,
    Finished,
}

pub struct DeflateEncoder<T> {
    inner: T,
    compressor: Compress,
    buffer: ByteBuffer,
    input_buffer: Bytes,
    state: EncoderState,
    /// This is the current amount of sent data and the window size used for intermittent flushing
    /// of the compression layer.
    flush_window: Option<(usize, usize)>,
}

pub struct DeflateEncoderBuilder<T> {
    inner: T,
    is_zlib: bool,
    buffer_size: usize,
    level: Level,
    flush_window: Option<usize>,
}

impl<T> DeflateEncoderBuilder<T> {
    pub const fn zlib(mut self, is_zlib: bool) -> Self {
        self.is_zlib = is_zlib;
        self
    }

    pub const fn level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    pub const fn buffer_size(mut self, buffer_size: usize) -> Self {
        self.buffer_size = buffer_size;
        self
    }

    /// If set, this is the number of bytes after which the compressor will be notified to flush
    /// some data, so the compression produces more steady output and a little earlier.
    pub const fn flush_window(mut self, flush_window: Option<usize>) -> Self {
        self.flush_window = flush_window;
        self
    }

    pub fn build(self) -> DeflateEncoder<T> {
        let level = match self.level {
            Level::Fastest => Compression::fast(),
            Level::Best => Compression::best(),
            Level::Default => Compression::new(3),
            Level::Precise(val) => Compression::new(val),
        };

        DeflateEncoder {
            inner: self.inner,
            compressor: Compress::new(level, self.is_zlib),
            buffer: ByteBuffer::with_capacity(self.buffer_size),
            input_buffer: Bytes::new(),
            state: EncoderState::Reading,
            flush_window: self.flush_window.map(|n| (0, n)),
        }
    }
}

impl<T> DeflateEncoder<T> {
    pub fn new(inner: T) -> Self {
        Self::builder(inner).build()
    }

    pub fn builder(inner: T) -> DeflateEncoderBuilder<T> {
        DeflateEncoderBuilder {
            inner,
            is_zlib: false,
            buffer_size: super::BUFFER_SIZE,
            level: Level::Default,
            flush_window: None,
        }
    }

    pub fn with_quality(inner: T, level: Level) -> Self {
        Self::builder(inner).level(level).build()
    }

    pub fn total_in(&self) -> u64 {
        self.compressor.total_in()
    }

    pub fn total_out(&self) -> u64 {
        self.compressor.total_out()
    }

    pub fn into_inner(self) -> T {
        self.inner
    }

    fn encode(
        &mut self,
        inbuf: &[u8],
        flush: FlushCompress,
    ) -> Result<(usize, flate2::Status), io::Error> {
        let old_in = self.compressor.total_in();
        let old_out = self.compressor.total_out();
        let res = self
            .compressor
            .compress(inbuf, self.buffer.get_free_mut_slice(), flush)?;
        let new_in = (self.compressor.total_in() - old_in) as usize;
        let new_out = (self.compressor.total_out() - old_out) as usize;
        self.buffer.add_size(new_out);

        Ok((new_in, res))
    }
}

impl DeflateEncoder<Vec<u8>> {
    // assume small files
    pub async fn compress_vec<R>(&mut self, reader: &mut R, size_hint: usize) -> Result<(), Error>
    where
        R: AsyncRead + Unpin,
    {
        let mut buffer = Vec::with_capacity(size_hint);
        reader.read_to_end(&mut buffer).await?;
        self.inner.reserve(size_hint); // should be enough since we want smalller files
        self.compressor
            .compress_vec(&buffer[..], &mut self.inner, FlushCompress::Finish)?;
        Ok(())
    }
}

impl<T: AsyncWrite + Unpin> DeflateEncoder<T> {
    pub async fn compress<R>(&mut self, reader: &mut R) -> Result<(), Error>
    where
        R: AsyncRead + Unpin,
    {
        let mut buffer = ByteBuffer::with_capacity(super::BUFFER_SIZE);
        let mut eof = false;
        loop {
            if !eof && !buffer.is_full() {
                let read = buffer.read_from_async(reader).await?;
                if read == 0 {
                    eof = true;
                }
            }
            let (read, _res) = self.encode(&buffer[..], FlushCompress::None)?;
            buffer.consume(read);

            self.inner.write_all(&self.buffer[..]).await?;
            self.buffer.clear();

            if buffer.is_empty() && eof {
                break;
            }
        }

        loop {
            let (_read, res) = self.encode(&[][..], FlushCompress::Finish)?;
            self.inner.write_all(&self.buffer[..]).await?;
            self.buffer.clear();
            if res == flate2::Status::StreamEnd {
                break;
            }
        }

        Ok(())
    }
}

impl<T, O> Stream for DeflateEncoder<T>
where
    T: Stream<Item = Result<O, io::Error>> + Unpin,
    O: Into<Bytes>,
{
    type Item = Result<Bytes, io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            match this.state {
                EncoderState::Reading => {
                    if let Some(res) = ready!(Pin::new(&mut this.inner).poll_next(cx)) {
                        let buf = res?;
                        this.input_buffer = buf.into();
                        this.state = EncoderState::Writing;
                    } else {
                        this.state = EncoderState::Flushing;
                    }
                }
                EncoderState::Writing => {
                    if this.input_buffer.is_empty() {
                        return Poll::Ready(Some(Err(io_format_err!("empty input during write"))));
                    }

                    // in stream mode we'd like to flush regularly as the library's inner buffer is
                    // quite huge...
                    let flush_mode = match &mut this.flush_window {
                        Some((at, window)) if *at >= *window => {
                            *at = 0;
                            FlushCompress::Sync
                        }
                        _ => FlushCompress::None,
                    };

                    let mut buf = this.input_buffer.split_off(0);
                    let (read, res) = this.encode(&buf[..], flush_mode)?;
                    this.input_buffer = buf.split_off(read);

                    if let Some((at, _window)) = &mut this.flush_window {
                        *at = at.saturating_add(read);
                    }

                    if this.input_buffer.is_empty() {
                        this.state = EncoderState::Reading;
                    }
                    if this.buffer.is_full() || res == flate2::Status::BufError {
                        let bytes = this.buffer.remove_data(this.buffer.len()).to_vec();
                        return Poll::Ready(Some(Ok(bytes.into())));
                    }
                }
                EncoderState::Flushing => {
                    let (_read, res) = this.encode(&[][..], FlushCompress::Finish)?;
                    if !this.buffer.is_empty() {
                        let bytes = this.buffer.remove_data(this.buffer.len()).to_vec();
                        return Poll::Ready(Some(Ok(bytes.into())));
                    }
                    if res == flate2::Status::StreamEnd {
                        this.state = EncoderState::Finished;
                    }
                }
                EncoderState::Finished => return Poll::Ready(None),
            }
        }
    }
}
