//! Client side TLS connection handling for `hyper`.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use hyper_util::client::legacy::connect::{Connected, Connection};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_openssl::SslStream;

/// Asynchronous stream, possibly encrypted and proxied
///
/// Useful for HTTP client implementations using hyper.
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
            MaybeTlsStream::Normal(s) => Pin::new(s).poll_read(cx, buf),
            MaybeTlsStream::Proxied(s) => Pin::new(s).poll_read(cx, buf),
            MaybeTlsStream::Secured(s) => Pin::new(s).poll_read(cx, buf),
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
            MaybeTlsStream::Normal(s) => Pin::new(s).poll_write(cx, buf),
            MaybeTlsStream::Proxied(s) => Pin::new(s).poll_write(cx, buf),
            MaybeTlsStream::Secured(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        match self.get_mut() {
            MaybeTlsStream::Normal(s) => Pin::new(s).poll_write_vectored(cx, bufs),
            MaybeTlsStream::Proxied(s) => Pin::new(s).poll_write_vectored(cx, bufs),
            MaybeTlsStream::Secured(s) => Pin::new(s).poll_write_vectored(cx, bufs),
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
            MaybeTlsStream::Normal(s) => Pin::new(s).poll_flush(cx),
            MaybeTlsStream::Proxied(s) => Pin::new(s).poll_flush(cx),
            MaybeTlsStream::Secured(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        match self.get_mut() {
            MaybeTlsStream::Normal(s) => Pin::new(s).poll_shutdown(cx),
            MaybeTlsStream::Proxied(s) => Pin::new(s).poll_shutdown(cx),
            MaybeTlsStream::Secured(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

// we need this for the hyper http client
impl<S: Connection + AsyncRead + AsyncWrite + Unpin> Connection for MaybeTlsStream<S> {
    fn connected(&self) -> Connected {
        match self {
            MaybeTlsStream::Normal(s) => s.connected(),
            MaybeTlsStream::Proxied(s) => s.connected().proxy(true),
            MaybeTlsStream::Secured(s) => {
                let connected = s.get_ref().connected();
                if s.ssl().selected_alpn_protocol() == Some(b"h2") {
                    connected.negotiated_h2()
                } else {
                    connected
                }
            }
        }
    }
}
