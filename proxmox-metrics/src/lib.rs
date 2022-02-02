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
/// Structured data for the metric server
pub struct MetricsData {
    /// The category of measurements
    pub measurement: String,
    /// A list of to attach to the measurements
    pub tags: HashMap<String, String>,
    /// The actual values to send. Only plain (not-nested) objects are supported at the moment.
    pub values: Value,
    /// The time of the measurement
    pub ctime: i64,
}

impl MetricsData {
    /// Convenient helper to create from references
    pub fn new<V: Serialize>(
        measurement: &str,
        tags: &[(&str, &str)],
        ctime: i64,
        values: V,
    ) -> Result<Self, Error> {
        let mut new_tags = HashMap::new();
        for (key, value) in tags {
            new_tags.insert(key.to_string(), value.to_string());
        }

        Ok(Self {
            measurement: measurement.to_string(),
            tags: new_tags,
            values: serde_json::to_value(values)?,
            ctime,
        })
    }
}

/// Helper to send a list of [MetricsData] to a list of [Metrics]
pub async fn send_data_to_channels(
    values: &[Arc<MetricsData>],
    connections: &[Metrics],
) -> Vec<Result<(), Error>> {
    let mut futures = Vec::with_capacity(connections.len());
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
/// You can send [MetricsData] by using [`Self::send_data()`], and to flush and
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
