//! Client side TLS connection handling for `hyper`.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use hyper::client::connect::{Connected, Connection};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_openssl::SslStream;

/// Asynchronous stream, possibly encrypted and proxied
///
/// Usefule for HTTP client implementations using hyper.
pub enum MaybeTlsStream<S> {
    Normal(S),
    Proxied(S),
    Secured(SslStream<S>),
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncRead for MaybeTlsStream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf,
    ) -> Poll<Result<(), io::Error>> {
        match self.get_mut() {
            MaybeTlsStream::Normal(ref mut s) => Pin::new(s).poll_read(cx, buf),
            MaybeTlsStream::Proxied(ref mut s) => Pin::new(s).poll_read(cx, buf),
            MaybeTlsStream::Secured(ref mut s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncWrite for MaybeTlsStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        match self.get_mut() {
            MaybeTlsStream::Normal(ref mut s) => Pin::new(s).poll_write(cx, buf),
            MaybeTlsStream::Proxied(ref mut s) => Pin::new(s).poll_write(cx, buf),
            MaybeTlsStream::Secured(ref mut s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        match self.get_mut() {
            MaybeTlsStream::Normal(ref mut s) => Pin::new(s).poll_write_vectored(cx, bufs),
            MaybeTlsStream::Proxied(ref mut s) => Pin::new(s).poll_write_vectored(cx, bufs),
            MaybeTlsStream::Secured(ref mut s) => Pin::new(s).poll_write_vectored(cx, bufs),
        }
    }

    fn is_write_vectored(&self) -> bool {
        match self {
            MaybeTlsStream::Normal(s) => s.is_write_vectored(),
            MaybeTlsStream::Proxied(s) => s.is_write_vectored(),
            MaybeTlsStream::Secured(s) => s.is_write_vectored(),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        match self.get_mut() {
            MaybeTlsStream::Normal(ref mut s) => Pin::new(s).poll_flush(cx),
            MaybeTlsStream::Proxied(ref mut s) => Pin::new(s).poll_flush(cx),
            MaybeTlsStream::Secured(ref mut s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        match self.get_mut() {
            MaybeTlsStream::Normal(ref mut s) => Pin::new(s).poll_shutdown(cx),
            MaybeTlsStream::Proxied(ref mut s) => Pin::new(s).poll_shutdown(cx),
            MaybeTlsStream::Secured(ref mut s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

// we need this for the hyper http client
impl<S: Connection + AsyncRead + AsyncWrite + Unpin> Connection for MaybeTlsStream<S> {
    fn connected(&self) -> Connected {
        match self {
            MaybeTlsStream::Normal(s) => s.connected(),
            MaybeTlsStream::Proxied(s) => s.connected().proxy(true),
            MaybeTlsStream::Secured(s) => s.get_ref().connected(),
        }
    }
}
