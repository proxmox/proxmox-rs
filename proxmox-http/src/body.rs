use std::{pin::Pin, task::Poll};

use anyhow::Error;
use bytes::Bytes;

use futures::ready;
use http_body_util::combinators::BoxBody;
use hyper::body::{Body as HyperBody, Frame, SizeHint};

// Partially copied and heavily based on reqwest 0.12 Body implementation from src/async_impl/body.rs
// Copyright (c) 2016-2025 Sean McArthur

/// Custom implementation of hyper::body::Body supporting either a "full" body that can return its
/// contents as byte sequence in one go, or "streaming" body that can be polled.
pub struct Body {
    inner: InnerBody,
}

enum InnerBody {
    Full(Bytes),
    Streaming(BoxBody<Bytes, Error>),
}

impl Body {
    /// Shortcut for creating an empty body instance with no data.
    pub fn empty() -> Self {
        Bytes::new().into()
    }

    /// Returns the body contents if it is a "full" body, None otherwise.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self.inner {
            InnerBody::Full(ref bytes) => Some(bytes),
            InnerBody::Streaming(_) => None,
        }
    }

    /// Returns the body contents as `Bytes` it is a "full" body, None otherwise.
    pub fn bytes(&self) -> Option<Bytes> {
        match self.inner {
            InnerBody::Full(ref bytes) => Some(bytes.clone()),
            InnerBody::Streaming(_) => None,
        }
    }

    pub fn wrap_stream<S>(stream: S) -> Body
    where
        S: futures::stream::TryStream + Send + 'static,

        S::Error: Into<Error>,

        Bytes: From<S::Ok>,
    {
        Body::stream(stream)
    }

    pub(crate) fn stream<S>(stream: S) -> Body
    where
        S: futures::stream::TryStream + Send + 'static,

        S::Error: Into<Error>,

        Bytes: From<S::Ok>,
    {
        use futures::TryStreamExt;

        use http_body::Frame;

        use http_body_util::StreamBody;

        let body = http_body_util::BodyExt::boxed(StreamBody::new(sync_wrapper::SyncStream::new(
            stream
                .map_ok(|d| Frame::data(Bytes::from(d)))
                .map_err(Into::into),
        )));

        Body {
            inner: InnerBody::Streaming(body),
        }
    }
}

impl HyperBody for Body {
    type Data = Bytes;

    type Error = Error;

    fn poll_frame(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
        match self.inner {
            InnerBody::Full(ref mut bytes) => {
                let res = bytes.split_off(0);
                if res.is_empty() {
                    Poll::Ready(None)
                } else {
                    Poll::Ready(Some(Ok(Frame::data(res))))
                }
            }
            InnerBody::Streaming(ref mut body) => {
                Poll::Ready(ready!(Pin::new(body).poll_frame(cx)).map(|opt_chunk| opt_chunk))
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        match self.inner {
            InnerBody::Full(ref bytes) => bytes.is_empty(),
            InnerBody::Streaming(ref box_body) => box_body.is_end_stream(),
        }
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        match self.inner {
            InnerBody::Full(ref bytes) => SizeHint::with_exact(bytes.len() as u64),
            InnerBody::Streaming(ref box_body) => box_body.size_hint(),
        }
    }
}

impl From<Bytes> for Body {
    fn from(value: Bytes) -> Self {
        Self {
            inner: InnerBody::Full(value),
        }
    }
}

impl From<Vec<u8>> for Body {
    fn from(value: Vec<u8>) -> Self {
        Bytes::from(value).into()
    }
}

impl From<String> for Body {
    fn from(value: String) -> Self {
        Bytes::copy_from_slice(value.as_bytes()).into()
    }
}
