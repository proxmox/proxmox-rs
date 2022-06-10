use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{bail, format_err, Error};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc;

mod influxdb;
#[doc(inline)]
pub use influxdb::{influxdb_http, influxdb_udp, test_influxdb_http, test_influxdb_udp};

#[derive(Clone)]
/// Structured data for the metric server.
pub struct MetricsData {
    /// The category of measurements.
    pub measurement: Cow<'static, str>,

    /// A list of to attach to the measurements.
    pub tags: HashMap<Cow<'static, str>, Cow<'static, str>>,

    /// The actual values to send. Only plain (not-nested) objects are supported at the moment.
    pub values: Value,

    /// The time of the measurement.
    pub ctime: i64,
}

impl MetricsData {
    /// Create a new metrics data entry.
    ///
    /// ```
    /// # use proxmox_metrics::MetricsData;
    /// # fn test(
    /// #     ctime: i64,
    /// #     stat: &'static str,
    /// #     nodename: String,
    /// # ) -> Result<(), anyhow::Error> {
    /// let data = MetricsData::new("memory", ctime, stat)?
    ///     .tag("object", "host")
    ///     .tag("host", nodename);
    /// #     Ok(())
    /// # }
    /// # test(0, "foo", "nodename".to_string()).unwrap();
    /// ```
    pub fn new<S, V>(measurement: S, ctime: i64, values: V) -> Result<Self, Error>
    where
        S: Into<Cow<'static, str>>,
        V: Serialize,
    {
        Ok(Self {
            values: serde_json::to_value(values)?,
            measurement: measurement.into(),
            tags: HashMap::new(),
            ctime,
        })
    }

    /// Add a tag.
    pub fn tag<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
    {
        self.tags.insert(key.into(), value.into());
        self
    }
}

/// Helper to send a list of [`MetricsData`] to a list of [`Metrics`].
pub async fn send_data_to_channels<'a, I: IntoIterator<Item = &'a Metrics>>(
    values: &[Arc<MetricsData>],
    connections: I,
) -> Vec<Result<(), Error>> {
    let connections = connections.into_iter();
    let mut futures = Vec::with_capacity(connections.size_hint().0);
    for connection in connections {
        futures.push(async move {
            for data in values {
                connection.send_data(Arc::clone(data)).await?
            }
            Ok::<(), Error>(())
        });
    }

    futures::future::join_all(futures).await
}

/// Represents connection to the metric server which can be used to send data
///
/// You can send [`MetricsData`] by using [`Self::send_data()`], and to flush and
/// finish the connection use [`Self::join`].
///
/// If dropped, it will abort the connection and not flush out buffered data.
pub struct Metrics {
    join_handle: Option<tokio::task::JoinHandle<Result<(), Error>>>,
    channel: Option<mpsc::Sender<Arc<MetricsData>>>,
}

impl Drop for Metrics {
    fn drop(&mut self) {
        if let Some(join_handle) = self.join_handle.take() {
            join_handle.abort();
        }
    }
}

impl Metrics {
    /// Closes the queue and waits for the connection to send all remaining data
    pub async fn join(mut self) -> Result<(), Error> {
        if let Some(channel) = self.channel.take() {
            drop(channel);
        }
        if let Some(join_handle) = self.join_handle.take() {
            join_handle.await?
        } else {
            bail!("internal error: no join_handle")
        }
    }

    /// Queues the given data to the metric server
    pub async fn send_data(&self, data: Arc<MetricsData>) -> Result<(), Error> {
        // return ok if we got no data to send
        if let Value::Object(map) = &data.values {
            if map.is_empty() {
                return Ok(());
            }
        }

        if let Some(channel) = &self.channel {
            channel
                .send(data)
                .await
                .map_err(|_| format_err!("receiver side closed"))?;
        } else {
            bail!("channel was already closed");
        }
        Ok(())
    }
}
