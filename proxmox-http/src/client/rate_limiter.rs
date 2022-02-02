use std::time::{Duration, Instant};
use std::convert::TryInto;

use anyhow::{bail, Error};

/// Rate limiter interface.
pub trait RateLimit {
    /// Update rate and bucket size
    fn update_rate(&mut self, rate: u64, bucket_size: u64);

    /// Returns the overall traffic (since started)
    fn traffic(&self) -> u64;

    /// Register traffic, returning a proposed delay to reach the
    /// expected rate.
    fn register_traffic(&mut self, current_time: Instant, data_len: u64) -> Duration;
}

/// Like [`RateLimit`], but does not require self to be mutable.
///
/// This is useful for types providing internal mutability (Mutex).
pub trait ShareableRateLimit: Send + Sync {
    fn update_rate(&self, rate: u64, bucket_size: u64);
    fn traffic(&self) -> u64;
    fn register_traffic(&self, current_time: Instant, data_len: u64) -> Duration;
}

/// IMPORTANT: We use this struct in shared memory, so please do not
/// change/modify the layout (do not add fields)
#[derive(Clone)]
#[repr(C)]
struct TbfState {
    traffic: u64, // overall traffic
    last_update: Instant,
    consumed_tokens: u64,
}

impl TbfState {

    const NO_DELAY: Duration = Duration::from_millis(0);

    fn refill_bucket(&mut self, rate: u64, current_time: Instant) {
        let time_diff = match current_time.checked_duration_since(self.last_update) {
            Some(duration) => duration.as_nanos(),
            None => return,
        };

        if time_diff == 0 { return; }

        self.last_update = current_time;

        let allowed_traffic = ((time_diff.saturating_mul(rate as u128)) / 1_000_000_000)
            .try_into().unwrap_or(u64::MAX);

        self.consumed_tokens = self.consumed_tokens.saturating_sub(allowed_traffic);
    }

    fn register_traffic(
        &mut self,
        rate: u64,
        bucket_size: u64,
        current_time: Instant,
        data_len: u64,
    ) -> Duration {
        self.refill_bucket(rate, current_time);

        self.traffic += data_len;
        self.consumed_tokens += data_len;

        if self.consumed_tokens <= bucket_size {
            return Self::NO_DELAY;
        }
        Duration::from_nanos((self.consumed_tokens - bucket_size).saturating_mul(1_000_000_000)/rate)
    }
}

/// Token bucket based rate limiter
///
/// IMPORTANT: We use this struct in shared memory, so please do not
/// change/modify the layout (do not add fields)
#[repr(C)]
pub struct RateLimiter {
    rate: u64, // tokens/second
    bucket_size: u64, // TBF bucket size
    state: TbfState,
}

impl RateLimiter {

    /// Creates a new instance, using [Instant::now] as start time.
    pub fn new(rate: u64, bucket_size: u64) -> Self {
        let start_time = Instant::now();
        Self::with_start_time(rate, bucket_size, start_time)
    }

    /// Creates a new instance with specified `rate`, `bucket_size` and `start_time`.
    pub fn with_start_time(rate: u64, bucket_size: u64, start_time: Instant) -> Self {
        Self {
            rate,
            bucket_size,
            state: TbfState {
                traffic: 0,
                last_update: start_time,
                // start with empty bucket (all tokens consumed)
                consumed_tokens: bucket_size,
            },
        }
    }
}

impl RateLimit for RateLimiter {

    fn update_rate(&mut self, rate: u64, bucket_size: u64) {
        self.rate = rate;

        if bucket_size < self.bucket_size && self.state.consumed_tokens > bucket_size {
            self.state.consumed_tokens = bucket_size; // start again
        }

        self.bucket_size = bucket_size;
    }

    fn traffic(&self) -> u64 {
        self.state.traffic
    }

    fn register_traffic(&mut self, current_time: Instant, data_len: u64) -> Duration {
        self.state.register_traffic(self.rate, self.bucket_size, current_time, data_len)
    }
}

impl <R: RateLimit + Send> ShareableRateLimit for std::sync::Mutex<R> {

    fn update_rate(&self, rate: u64, bucket_size: u64) {
        self.lock().unwrap().update_rate(rate, bucket_size);
    }

    fn traffic(&self) -> u64 {
        self.lock().unwrap().traffic()
    }

    fn register_traffic(&self, current_time: Instant, data_len: u64) -> Duration {
        self.lock().unwrap().register_traffic(current_time, data_len)
    }
}


/// Array of rate limiters.
///
/// A group of rate limiters with same configuration.
pub struct RateLimiterVec {
    rate: u64, // tokens/second
    bucket_size: u64, // TBF bucket size
    state: Vec<TbfState>,
}

impl RateLimiterVec {

    /// Creates a new instance, using [Instant::now] as start time.
    pub fn new(group_size: usize, rate: u64, bucket_size: u64) -> Self {
        let start_time = Instant::now();
        Self::with_start_time(group_size, rate, bucket_size, start_time)
    }

    /// Creates a new instance with specified `rate`, `bucket_size` and `start_time`.
    pub fn with_start_time(group_size: usize, rate: u64, bucket_size: u64, start_time: Instant) -> Self {
        let state = TbfState {
            traffic: 0,
            last_update: start_time,
            // start with empty bucket (all tokens consumed)
            consumed_tokens: bucket_size,
        };
        Self {
            rate,
            bucket_size,
            state: vec![state; group_size],
        }
    }

    /// Return the number of TBF entries (group_size)
    pub fn len(&self) -> usize {
        self.state.len()
    }

    /// Traffic for the specified index
    pub fn traffic(&self, index: usize) -> Result<u64, Error> {
        if index >= self.state.len() {
            bail!("RateLimiterVec::traffic - index out of range");
        }
        Ok(self.state[index].traffic)
    }

    /// Register traffic at the specified index
    pub fn register_traffic(&mut self, index: usize, current_time: Instant, data_len: u64) -> Result<Duration, Error> {
        if index >= self.state.len() {
            bail!("RateLimiterVec::register_traffic - index out of range");
        }

        Ok(self.state[index].register_traffic(self.rate, self.bucket_size, current_time, data_len))
    }
}
