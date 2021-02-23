use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};

pub struct AsyncBlockingReader<R> {
    inner: R,
}

impl<W> AsyncBlockingReader<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &W {
        &self.inner
    }
}

pub struct AsyncBlockingWriter<W> {
    inner: W,
    seek_pos: u64,
}

impl<W> AsyncBlockingWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner, seek_pos: 0 }
    }

    pub fn inner(&self) -> &W {
        &self.inner
    }
}

impl<R: std::io::Read + Unpin> AsyncRead for AsyncBlockingReader<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = Pin::get_mut(self);
        let mut read_buf = buf.initialize_unfilled();
        Poll::Ready(match this.inner.read(&mut read_buf) {
            Ok(len) => {
                buf.advance(len);
                Ok(())
            }
            Err(err) => Err(err),
        })
    }
}

impl<R: std::io::Write + Unpin> AsyncWrite for AsyncBlockingWriter<R> {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = Pin::get_mut(self);
        Poll::Ready(match this.inner.write(buf) {
            Ok(len) => Ok(len),
            Err(err) => Err(err),
        })
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = Pin::get_mut(self);
        Poll::Ready(this.inner.flush())
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl<R: std::io::Seek + Unpin> AsyncSeek for AsyncBlockingWriter<R> {
    fn start_seek(self: Pin<&mut Self>, position: std::io::SeekFrom) -> std::io::Result<()> {
        let this = Pin::get_mut(self);
        this.seek_pos = this.inner.seek(position)?;
        Ok(())
    }

    fn poll_complete(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        let this = Pin::get_mut(self);
        Poll::Ready(Ok(this.seek_pos))
    }
}
