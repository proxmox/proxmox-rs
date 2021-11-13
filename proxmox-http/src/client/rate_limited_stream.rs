use std::pin::Pin;
use std::marker::Unpin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::io::IoSlice;

use futures::Future;
use tokio::io::{ReadBuf, AsyncRead, AsyncWrite};
use tokio::time::Sleep;
use hyper::client::connect::{Connection, Connected};

use std::task::{Context, Poll};

use super::{RateLimit, RateLimiter};

/// A rate limited stream using [RateLimiter]
pub struct RateLimitedStream<S> {
    read_limiter: Option<Arc<Mutex<RateLimiter>>>,
    read_delay: Option<Pin<Box<Sleep>>>,
    write_limiter: Option<Arc<Mutex<RateLimiter>>>,
    write_delay: Option<Pin<Box<Sleep>>>,
    update_limiter_cb: Option<Box<dyn Fn() -> (Option<Arc<Mutex<RateLimiter>>>, Option<Arc<Mutex<RateLimiter>>>) + Send>>,
    last_limiter_update: Instant,
    stream: S,
}

impl RateLimitedStream<tokio::net::TcpStream> {
    pub fn peer_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.stream.peer_addr()
    }
}

impl <S> RateLimitedStream<S> {

    /// Creates a new instance with reads and writes limited to the same `rate`.
    pub fn new(stream: S, rate: u64, bucket_size: u64) -> Self {
        let now = Instant::now();
        let read_limiter = Arc::new(Mutex::new(RateLimiter::with_start_time(rate, bucket_size, now)));
        let write_limiter = Arc::new(Mutex::new(RateLimiter::with_start_time(rate, bucket_size, now)));
        Self::with_limiter(stream, Some(read_limiter), Some(write_limiter))
    }

    /// Creates a new instance with specified [RateLimiters] for reads and writes.
    pub fn with_limiter(
        stream: S,
        read_limiter: Option<Arc<Mutex<RateLimiter>>>,
        write_limiter: Option<Arc<Mutex<RateLimiter>>>,
    ) -> Self {
        Self {
            read_limiter,
            read_delay: None,
            write_limiter,
            write_delay: None,
            update_limiter_cb: None,
            last_limiter_update: Instant::now(),
            stream,
        }
    }

    /// Creates a new instance with limiter update callback.
    ///
    /// The fuction is called every minute to update/change the used limiters.
    ///
    /// Note: This function is called within an async context, so it
    /// should be fast and must not block.
    pub fn with_limiter_update_cb<F: Fn() -> (Option<Arc<Mutex<RateLimiter>>>, Option<Arc<Mutex<RateLimiter>>>) + Send + 'static>(
        stream: S,
        update_limiter_cb: F,
    ) -> Self {
        let (read_limiter, write_limiter) = update_limiter_cb();
        Self {
            read_limiter,
            read_delay: None,
            write_limiter,
            write_delay: None,
            update_limiter_cb: Some(Box::new(update_limiter_cb)),
            last_limiter_update: Instant::now(),
            stream,
        }
    }

    fn update_limiters(&mut self) {
        if let Some(ref update_limiter_cb) = self.update_limiter_cb {
            if self.last_limiter_update.elapsed().as_secs() >= 5 {
                self.last_limiter_update = Instant::now();
                let (read_limiter, write_limiter) = update_limiter_cb();
                self.read_limiter = read_limiter;
                self.write_limiter = write_limiter;
            }
        }
    }
}

fn register_traffic(
    limiter: &Mutex<RateLimiter>,
    count: usize,
) -> Option<Pin<Box<Sleep>>>{

    const MIN_DELAY: Duration = Duration::from_millis(10);

    let now = Instant::now();
    let delay = limiter.lock().unwrap()
        .register_traffic(now, count as u64);
    if delay >= MIN_DELAY {
        let sleep = tokio::time::sleep(delay);
        Some(Box::pin(sleep))
    } else {
        None
    }
}

fn delay_is_ready(delay: &mut Option<Pin<Box<Sleep>>>, ctx: &mut Context<'_>) -> bool {
    match delay {
        Some(ref mut future) => {
            future.as_mut().poll(ctx).is_ready()
        }
        None => true,
    }
}

impl <S: AsyncWrite + Unpin> AsyncWrite for RateLimitedStream<S> {

    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8]
    ) -> Poll<Result<usize, std::io::Error>> {
        let this = self.get_mut();

        let is_ready = delay_is_ready(&mut this.write_delay, ctx);

        if !is_ready { return Poll::Pending; }

        this.write_delay = None;

        this.update_limiters();

        let result = Pin::new(&mut this.stream).poll_write(ctx, buf);

        if let Some(ref limiter) = this.write_limiter {
            if let Poll::Ready(Ok(count)) = result {
                this.write_delay = register_traffic(limiter, count);
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
        bufs: &[IoSlice<'_>]
    ) -> Poll<Result<usize, std::io::Error>> {
        let this = self.get_mut();

        let is_ready = delay_is_ready(&mut this.write_delay, ctx);

        if !is_ready { return Poll::Pending; }

        this.write_delay = None;

        this.update_limiters();

        let result = Pin::new(&mut this.stream).poll_write_vectored(ctx, bufs);

        if let Some(ref limiter) = this.write_limiter {
            if let Poll::Ready(Ok(count)) = result {
                this.write_delay = register_traffic(limiter, count);
            }
        }

        result
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>
    ) -> Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();
        Pin::new(&mut this.stream).poll_flush(ctx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>
    ) -> Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();
        Pin::new(&mut this.stream).poll_shutdown(ctx)
    }
}

impl <S: AsyncRead + Unpin> AsyncRead for RateLimitedStream<S> {

    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();

        let is_ready = delay_is_ready(&mut this.read_delay, ctx);

        if !is_ready { return Poll::Pending; }

        this.read_delay = None;

        this.update_limiters();

        let filled_len = buf.filled().len();
        let result = Pin::new(&mut this.stream).poll_read(ctx, buf);

        if let Some(ref read_limiter) = this.read_limiter {
            if let Poll::Ready(Ok(())) = &result {
                let count = buf.filled().len() - filled_len;
                this.read_delay = register_traffic(read_limiter, count);
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
