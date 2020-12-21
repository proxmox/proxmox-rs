//! Common extensions for Futures
use anyhow::Error;
use futures::future::{select, Either, FutureExt};
use std::future::Future;
use std::time::Duration;
use tokio::time::delay_for;

impl<T> TimeoutFutureExt for T where T: Future {}

/// Implements a timeout for futures, automatically aborting them if the timeout runs out before
/// the base future completes.
pub trait TimeoutFutureExt: Future {
    /// Returned Future returns 'None' in case the timeout was reached, otherwise the original
    /// return value.
    fn or_timeout<'a>(
        self,
        timeout: Duration,
    ) -> Box<dyn Future<Output = Option<Self::Output>> + Unpin + Send + 'a>
    where
        Self: Sized + Unpin + Send + 'a,
    {
        let timeout_fut = delay_for(timeout);
        Box::new(select(self, timeout_fut).map(|res| match res {
            Either::Left((result, _)) => Some(result),
            Either::Right(((), _)) => None,
        }))
    }

    /// Returned Future returns either the original result, or `Err<err>` in case the timeout is
    /// reached. Basically a shorthand to flatten a future that returns a `Result<_, Error>` with a
    /// timeout. The base Future can return any kind of Error that can be made into an
    /// `anyhow::Error`.
    fn or_timeout_err<'a, O, E>(
        self,
        timeout: Duration,
        err: Error,
    ) -> Box<dyn Future<Output = Result<O, Error>> + Unpin + Send + 'a>
    where
        Self: Sized + Unpin + Send + 'a,
        Self::Output: Into<Result<O, E>>,
        E: Into<Error> + std::error::Error + Send + Sync + 'static,
    {
        Box::new(self.or_timeout(timeout).map(|res| match res {
            Some(res) => res.into().map_err(Error::from),
            None => Err(err),
        }))
    }
}
