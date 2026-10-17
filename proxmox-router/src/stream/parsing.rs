use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Poll, ready};

use anyhow::{Context as _, Error, format_err};
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
                            if !read_record(&mut reader, &mut linebuf).await? {
                                return Ok(None);
                            }
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

/// Read the next record of an RFC 7464 JSON text sequence into `buf`, without its framing.
/// Returns `false` at the end of the stream.
///
/// A record is a record separator (`\x1E`), the JSON text and a line feed. Waiting for the next
/// record separator holds the latest record back for as long as the sender has nothing new to
/// write, so a record is also complete at a line feed that ends a whole JSON text. Line feeds
/// inside a JSON text, as in pretty-printed output, do not end the record. A record separator
/// always ends it, so a truncated JSON text does not swallow the record that follows.
async fn read_record<R>(reader: &mut R, buf: &mut Vec<u8>) -> io::Result<bool>
where
    R: AsyncBufRead + Unpin,
{
    buf.clear();
    let mut read_any = false;
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            break;
        }
        read_any = true;
        let (used, done) = match available.iter().position(|&b| b == b'\n' || b == b'\x1E') {
            None => {
                buf.extend_from_slice(available);
                (available.len(), false)
            }
            Some(pos) if available[pos] == b'\n' => {
                buf.extend_from_slice(&available[..=pos]);
                (pos + 1, is_complete_json(buf))
            }
            Some(pos) => {
                buf.extend_from_slice(&available[..pos]);
                if buf.iter().all(u8::is_ascii_whitespace) {
                    // the separator starts this record
                    buf.clear();
                    (pos + 1, false)
                } else {
                    // the separator starts the next record, leave it for the next call
                    (pos, true)
                }
            }
        };
        reader.consume_unpin(used);
        if done {
            break;
        }
    }
    while buf.last().is_some_and(u8::is_ascii_whitespace) {
        buf.pop();
    }
    Ok(read_any)
}

/// Whether `text` holds a whole JSON text rather than the start of one.
fn is_complete_json(text: &[u8]) -> bool {
    match serde_json::from_slice::<serde::de::IgnoredAny>(text) {
        Ok(_) => true,
        // a syntax error is reported by the caller, more data would not fix it
        Err(err) => !err.is_eof(),
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
        let Self { reader, buf_at } = Pin::into_inner(self);
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

#[cfg(test)]
mod tests {
    use futures::{FutureExt, StreamExt};
    use serde_json::Value;

    use super::*;

    /// A body that delivers `chunks` and then stays open, as a response does while the sender has
    /// nothing new to write.
    fn open_body(chunks: &[&'static [u8]]) -> Body {
        let chunks: Vec<Result<&'static [u8], Error>> = chunks.iter().map(|c| Ok(*c)).collect();
        Body::wrap_stream(futures::stream::iter(chunks).chain(futures::stream::pending()))
    }

    /// A body that delivers `chunks` and then ends.
    fn closed_body(chunks: &[&'static [u8]]) -> Body {
        let chunks: Vec<Result<&'static [u8], Error>> = chunks.iter().map(|c| Ok(*c)).collect();
        Body::wrap_stream(futures::stream::iter(chunks))
    }

    /// The records read from `body` without waiting for more data, and whether the body ended.
    fn read_available(body: Body) -> (Vec<String>, bool) {
        let mut records = JsonRecords::<Value>::from_body(body);
        let mut read = Vec::new();
        loop {
            match records.next().now_or_never() {
                None => return (read, false),
                Some(None) => return (read, true),
                Some(Some(Ok(Record::Data(data)))) => read.push(data.to_string()),
                Some(Some(Ok(Record::Error(error)))) => read.push(format!("error: {error}")),
                Some(Some(Err(err))) => read.push(format!("failed: {err}")),
            }
        }
    }

    #[test]
    fn a_complete_record_arrives_while_the_body_stays_open() {
        assert_eq!(
            read_available(open_body(&[b"\x1E{\"data\":1}\n"])),
            (vec!["1".to_string()], false)
        );
        assert_eq!(
            read_available(open_body(&[b"\x1E{\"error\":\"failed\"}\n"])),
            (vec!["error: \"failed\"".to_string()], false)
        );
    }

    #[test]
    fn records_split_across_chunks_arrive_once_complete() {
        let chunks: &[&[u8]] = &[
            b"\x1E{\"data\":1}\n\x1E{\"da",
            b"ta\":2}",
            b"\n",
            b"\x1E",
            b"{\"data\":3}\n",
        ];
        assert_eq!(
            read_available(open_body(chunks)),
            (
                vec!["1".to_string(), "2".to_string(), "3".to_string()],
                false
            )
        );
        // a record is held back until its line feed arrives
        assert_eq!(
            read_available(open_body(&[b"\x1E{\"data\":1}\n\x1E{\"data\":2}"])),
            (vec!["1".to_string()], false)
        );
    }

    #[test]
    fn line_feeds_inside_a_json_text_do_not_end_its_record() {
        assert_eq!(
            read_available(open_body(&[b"\x1E{\n  \"data\": [\n    1\n", b"  ]\n}\n"])),
            (vec!["[1]".to_string()], false)
        );
    }

    #[test]
    fn a_separator_ends_a_truncated_record() {
        assert_eq!(
            read_available(open_body(&[b"\x1E{\"da\x1E{\"data\":2}\n"])),
            (
                vec![
                    "failed: bad json in record element: \"{\\\"da\"".to_string(),
                    "2".to_string()
                ],
                false
            )
        );
    }

    #[test]
    fn a_record_cut_off_by_the_end_of_the_body_keeps_its_last_byte() {
        assert_eq!(
            read_available(closed_body(&[b"\x1E{\"data\":1}\n\x1E{\"data\":2}"])),
            (vec!["1".to_string(), "2".to_string()], true)
        );
    }

    #[test]
    fn a_record_ends_with_its_line_feed_at_the_end_of_the_body() {
        assert_eq!(
            read_available(closed_body(&[b"\x1E{\"data\":1}\n\x1E{\"data\":2}\n"])),
            (vec!["1".to_string(), "2".to_string()], true)
        );
    }
}
