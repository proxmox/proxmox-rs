use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{ready, Poll};

use anyhow::{format_err, Context as _, Error};
use bytes::Bytes;
use futures::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, BufReader};
use serde::Deserialize;

use proxmox_http::Body;

use super::Record;

pub struct Records<R = BodyBufReader>
where
    R: Send + Sync,
{
    inner: RecordsInner<R>,
}

impl<R: Send + Sync> Records<R> {
    /// Create a *new buffered reader* for to create a record stream from an [`AsyncRead`].
    /// Note: If the underlying type already implements [`AsyncBufRead`], use [`Records::from`]
    /// instead!
    pub fn new<T>(reader: T) -> Records<BufReader<T>>
    where
        T: AsyncRead + Send + Sync + Unpin + 'static,
    {
        BufReader::new(reader).into()
    }
}

impl<R> Records<R>
where
    R: AsyncBufRead + Send + Sync + Unpin + 'static,
{
    pub fn json<T>(self) -> JsonRecords<T, R>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.inner.into()
    }
}

impl Records<BodyBufReader> {
    pub fn from_body(body: Body) -> Self {
        Self::from(BodyBufReader::from(body))
    }
}

impl<R> From<R> for Records<R>
where
    R: AsyncBufRead + Send + Sync + Unpin + 'static,
{
    fn from(reader: R) -> Self {
        Self {
            inner: reader.into(),
        }
    }
}

enum RecordsInner<R: Send + Sync> {
    New(R),
    #[allow(clippy::type_complexity)]
    Reading(Pin<Box<dyn Future<Output = io::Result<Option<(Vec<u8>, R)>>> + Send + Sync>>),
    Done,
}

impl<R> From<R> for RecordsInner<R>
where
    R: AsyncBufRead + Send + Sync + Unpin + 'static,
{
    fn from(reader: R) -> Self {
        Self::New(reader)
    }
}

impl<R> futures::Stream for RecordsInner<R>
where
    R: AsyncBufRead + Send + Sync + Unpin + 'static,
{
    type Item = io::Result<Vec<u8>>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<io::Result<Vec<u8>>>> {
        loop {
            return match std::mem::replace(&mut *self, Self::Done) {
                Self::New(mut reader) => {
                    let fut = Box::pin(async move {
                        let mut linebuf = Vec::new();
                        loop {
                            if reader.read_until(b'\x1E', &mut linebuf).await? == 0 {
                                return Ok(None);
                            }
                            linebuf.pop(); // pop off the record separator
                            if linebuf.is_empty() {
                                continue;
                            }
                            return Ok(Some((linebuf, reader)));
                        }
                    });
                    *self = Self::Reading(fut);
                    continue;
                }
                Self::Reading(mut fut) => match fut.as_mut().poll(cx) {
                    Poll::Ready(Ok(None)) => Poll::Ready(None),
                    Poll::Ready(Ok(Some((data, reader)))) => {
                        *self = Self::New(reader);
                        Poll::Ready(Some(Ok(data)))
                    }
                    Poll::Ready(Err(err)) => {
                        *self = Self::Done;
                        Poll::Ready(Some(Err(err)))
                    }
                    Poll::Pending => {
                        *self = Self::Reading(fut);
                        Poll::Pending
                    }
                },
                Self::Done => Poll::Ready(None),
            };
        }
    }
}

pub struct JsonRecords<T, R = BodyBufReader>
where
    R: Send + Sync,
{
    inner: JsonRecordsInner<T, R>,
}

impl<T, R> JsonRecords<T, R>
where
    R: Send + Sync,
{
    pub fn from_vec(list: Vec<T>) -> Self {
        Self {
            inner: JsonRecordsInner::Fixed(list.into_iter()),
        }
    }
}

enum JsonRecordsInner<T, R: Send + Sync> {
    Stream(RecordsInner<R>),
    Fixed(std::vec::IntoIter<T>),
}

impl<T, R> From<R> for JsonRecords<T, R>
where
    R: AsyncBufRead + Send + Sync + Unpin + 'static,
    T: for<'de> Deserialize<'de>,
{
    fn from(reader: R) -> Self {
        Self::from(RecordsInner::from(reader))
    }
}

impl<T> JsonRecords<T, BodyBufReader>
where
    T: for<'de> Deserialize<'de>,
{
    pub fn from_body(body: Body) -> Self {
        Self::from(BodyBufReader::from(body))
    }
}

impl<T, R> From<RecordsInner<R>> for JsonRecords<T, R>
where
    R: AsyncBufRead + Send + Sync + Unpin + 'static,
    T: for<'de> Deserialize<'de>,
{
    fn from(inner: RecordsInner<R>) -> Self {
        Self {
            inner: JsonRecordsInner::Stream(inner),
        }
    }
}

impl<T, R> futures::Stream for JsonRecords<T, R>
where
    R: AsyncBufRead + Send + Sync + Unpin + 'static,
    T: Unpin + for<'de> Deserialize<'de>,
{
    type Item = Result<Record<T>, Error>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Record<T>, Error>>> {
        let this = match &mut self.get_mut().inner {
            JsonRecordsInner::Stream(this) => this,
            JsonRecordsInner::Fixed(iter) => {
                return Poll::Ready(iter.next().map(|item| Ok(Record::Data(item))));
            }
        };

        loop {
            match ready!(Pin::new(&mut *this).poll_next(cx)) {
                None => return Poll::Ready(None),
                Some(Err(err)) => return Poll::Ready(Some(Err(err.into()))),
                Some(Ok(data)) => {
                    let data = std::str::from_utf8(&data)
                        .map_err(|_| format_err!("non-utf8 json data in record element"))?
                        .trim();
                    if data.is_empty() {
                        continue;
                    }
                    return Poll::Ready(Some(
                        serde_json::from_str(data)
                            .with_context(|| format!("bad json in record element: {data:?}")),
                    ));
                }
            }
        }
    }
}

/// An adapter to turn a [`hyper::Body`] into an `AsyncRead`/`AsyncBufRead` for use with the
/// [`Records`]` type.
pub struct BodyBufReader {
    reader: Option<Body>,
    buf_at: Option<(Bytes, usize)>,
}

impl BodyBufReader {
    pub fn records(self) -> Records<Self> {
        self.into()
    }

    pub fn json_records<T>(self) -> JsonRecords<T, Self>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.into()
    }

    pub fn new(body: Body) -> Self {
        Self {
            reader: Some(body),
            buf_at: None,
        }
    }
}

impl From<Body> for BodyBufReader {
    fn from(body: Body) -> Self {
        Self::new(body)
    }
}

impl AsyncRead for BodyBufReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        use std::io::Read;
        let mut current_data = ready!(self.as_mut().poll_fill_buf(cx))?;
        let nread = current_data.read(buf)?;
        self.consume(nread);
        Poll::Ready(Ok(nread))
    }
}

impl AsyncBufRead for BodyBufReader {
    fn poll_fill_buf(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<io::Result<&[u8]>> {
        use hyper::body::Body as HyperBody;
        let Self {
            ref mut reader,
            ref mut buf_at,
        } = Pin::into_inner(self);
        loop {
            // If we currently have a buffer, use it:
            if let Some((buf, at)) = buf_at {
                return Poll::Ready(Ok(&buf[*at..]));
            };

            let result = match reader {
                None => return Poll::Ready(Ok(&[])),
                Some(reader) => ready!(Pin::new(reader).poll_frame(cx)),
            };

            match result {
                Some(Ok(bytes)) => {
                    *buf_at = Some((
                        bytes
                            .into_data()
                            .map_err(|_frame| io::Error::other("Failed to read frame from body"))?,
                        0,
                    ));
                }
                Some(Err(err)) => {
                    *reader = None;
                    return Poll::Ready(Err(io::Error::other(err)));
                }
                None => {
                    *reader = None;
                    return Poll::Ready(Ok(&[]));
                }
            }
        }
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        if let Some((buf, at)) = self.buf_at.as_mut() {
            *at = (*at + amt).min(buf.len());
            if *at == buf.len() {
                self.buf_at = None;
            }
        }
    }
}
