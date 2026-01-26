use std::future::Future;
use std::io::IoSlice;
use std::marker::Unpin;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hyper_util::client::legacy::connect::{Connected, Connection};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::Sleep;

use std::task::{Context, Poll};

use proxmox_rate_limiter::{RateLimiter, ShareableRateLimit};

type SharedRateLimit = Arc<dyn ShareableRateLimit>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RateLimiterTag {
    User(String),
}

pub type RateLimiterTags = Vec<RateLimiterTag>;

#[derive(Clone, Debug)]
pub struct RateLimiterTagsHandle {
    tags: Arc<Mutex<RateLimiterTags>>,
    dirty: Arc<AtomicBool>,
}

impl RateLimiterTagsHandle {
    fn new() -> Self {
        Self {
            tags: Arc::new(Mutex::new(Vec::new())),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, RateLimiterTags> {
        self.tags.lock().unwrap()
    }

    pub fn set_tags(&self, tags: RateLimiterTags) {
        *self.tags.lock().unwrap() = tags;
        self.dirty.store(true, Ordering::Release);
    }
}

pub type RateLimiterCallback =
    dyn Fn(&[RateLimiterTag]) -> (Option<SharedRateLimit>, Option<SharedRateLimit>) + Send;

/// A rate limited stream using [RateLimiter]
pub struct RateLimitedStream<S> {
    read_limiter: Option<SharedRateLimit>,
    read_delay: Option<Pin<Box<Sleep>>>,
    write_limiter: Option<SharedRateLimit>,
    write_delay: Option<Pin<Box<Sleep>>>,
    update_limiter_cb: Option<Box<RateLimiterCallback>>,
    last_limiter_update: Instant,
    tag_handle: Option<RateLimiterTagsHandle>,
    stream: S,
}

impl<S> RateLimitedStream<S> {
    /// Creates a new instance with reads and writes limited to the same `rate`.
    pub fn new(stream: S, rate: u64, bucket_size: u64) -> Self {
        let now = Instant::now();
        let read_limiter = RateLimiter::with_start_time(rate, bucket_size, now);
        let read_limiter: SharedRateLimit = Arc::new(Mutex::new(read_limiter));
        let write_limiter = RateLimiter::with_start_time(rate, bucket_size, now);
        let write_limiter: SharedRateLimit = Arc::new(Mutex::new(write_limiter));
        Self::with_limiter(stream, Some(read_limiter), Some(write_limiter))
    }

    /// Creates a new instance with specified [`RateLimiter`s](RateLimiter) for reads and writes.
    pub fn with_limiter(
        stream: S,
        read_limiter: Option<SharedRateLimit>,
        write_limiter: Option<SharedRateLimit>,
    ) -> Self {
        Self {
            read_limiter,
            read_delay: None,
            write_limiter,
            write_delay: None,
            update_limiter_cb: None,
            last_limiter_update: Instant::now(),
            tag_handle: None,
            stream,
        }
    }

    /// Creates a new instance with limiter update callback.
    ///
    /// The function is called every minute to update/change the used limiters.
    ///
    /// Note: This function is called within an async context, so it
    /// should be fast and must not block.
    pub fn with_limiter_update_cb<
        F: Fn(&[RateLimiterTag]) -> (Option<SharedRateLimit>, Option<SharedRateLimit>)
            + Send
            + 'static,
    >(
        stream: S,
        update_limiter_cb: F,
    ) -> Self {
        let tag_handle = Some(RateLimiterTagsHandle::new());
        let (read_limiter, write_limiter) = update_limiter_cb(&[]);
        Self {
            read_limiter,
            read_delay: None,
            write_limiter,
            write_delay: None,
            update_limiter_cb: Some(Box::new(update_limiter_cb)),
            last_limiter_update: Instant::now(),
            tag_handle,
            stream,
        }
    }

    fn update_limiters(&mut self) {
        if let Some(ref update_limiter_cb) = self.update_limiter_cb {
            let mut force_update = false;

            if let Some(ref handle) = self.tag_handle {
                if handle.dirty.swap(false, Ordering::Acquire) {
                    force_update = true;
                }
            }

            if force_update || self.last_limiter_update.elapsed().as_secs() >= 5 {
                self.last_limiter_update = Instant::now();
                let (read_limiter, write_limiter) = if let Some(ref handle) = self.tag_handle {
                    let tags = handle.lock();
                    update_limiter_cb(&tags)
                } else {
                    update_limiter_cb(&[])
                };
                self.read_limiter = read_limiter;
                self.write_limiter = write_limiter;
            }
        }
    }

    pub fn inner(&self) -> &S {
        &self.stream
    }

    pub fn inner_mut(&mut self) -> &mut S {
        &mut self.stream
    }

    pub fn tag_handle(&self) -> Option<Arc<Mutex<RateLimiterTags>>> {
        self.tag_handle
            .as_ref()
            .map(|handle| Arc::clone(&handle.tags))
    }

    pub fn rate_limiter_tags_handle(&self) -> Option<&RateLimiterTagsHandle> {
        self.tag_handle.as_ref()
    }
}

fn register_traffic(limiter: &dyn ShareableRateLimit, count: usize) -> Option<Pin<Box<Sleep>>> {
    const MIN_DELAY: Duration = Duration::from_millis(10);

    let now = Instant::now();
    let delay = limiter.register_traffic(now, count as u64);
    if delay >= MIN_DELAY {
        let sleep = tokio::time::sleep(delay);
        Some(Box::pin(sleep))
    } else {
        None
    }
}

fn delay_is_ready(delay: &mut Option<Pin<Box<Sleep>>>, ctx: &mut Context<'_>) -> bool {
    match delay {
        Some(future) => future.as_mut().poll(ctx).is_ready(),
        None => true,
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for RateLimitedStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let this = self.get_mut();

        let is_ready = delay_is_ready(&mut this.write_delay, ctx);

        if !is_ready {
            return Poll::Pending;
        }

        this.write_delay = None;

        this.update_limiters();

        let result = Pin::new(&mut this.stream).poll_write(ctx, buf);

        if let Some(ref mut limiter) = this.write_limiter {
            if let Poll::Ready(Ok(count)) = result {
                this.write_delay = register_traffic(limiter.as_ref(), count);
            }
        }

        result
    }

    fn is_write_vectored(&self) -> bool {
        self.stream.is_write_vectored()
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        let this = self.get_mut();

        let is_ready = delay_is_ready(&mut this.write_delay, ctx);

        if !is_ready {
            return Poll::Pending;
        }

        this.write_delay = None;

        this.update_limiters();

        let result = Pin::new(&mut this.stream).poll_write_vectored(ctx, bufs);

        if let Some(ref limiter) = this.write_limiter {
            if let Poll::Ready(Ok(count)) = result {
                this.write_delay = register_traffic(limiter.as_ref(), count);
            }
        }

        result
    }

    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();
        Pin::new(&mut this.stream).poll_flush(ctx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();
        Pin::new(&mut this.stream).poll_shutdown(ctx)
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for RateLimitedStream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();

        let is_ready = delay_is_ready(&mut this.read_delay, ctx);

        if !is_ready {
            return Poll::Pending;
        }

        this.read_delay = None;

        this.update_limiters();

        let filled_len = buf.filled().len();
        let result = Pin::new(&mut this.stream).poll_read(ctx, buf);

        if let Some(ref read_limiter) = this.read_limiter {
            if let Poll::Ready(Ok(())) = &result {
                let count = buf.filled().len() - filled_len;
                this.read_delay = register_traffic(read_limiter.as_ref(), count);
            }
        }

        result
    }
}

// we need this for the hyper http client
impl<S: Connection + AsyncRead + AsyncWrite + Unpin> Connection for RateLimitedStream<S> {
    fn connected(&self) -> Connected {
        self.stream.connected()
    }
}
