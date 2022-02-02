use std::sync::Arc;

use anyhow::Error;
use tokio::sync::mpsc;

use proxmox_async::net::udp;

use crate::influxdb::utils;
use crate::{Metrics, MetricsData};

struct InfluxDbUdp {
    address: String,
    conn: Option<tokio::net::UdpSocket>,
    mtu: u16,
    data: String,
    channel: mpsc::Receiver<Arc<MetricsData>>,
}

/// Tests the connection to the given influxdb udp server.
pub async fn test_influxdb_udp(address: &str) -> Result<(), Error> {
    udp::connect(address).await?;
    Ok(())
}

/// Get a [`Metrics`] handle for an influxdb server accessed via UDP.
///
/// `address` must be in the format of `ip_or_hostname:port`
pub fn influxdb_udp(address: &str, mtu: Option<u16>) -> Metrics {
    let (tx, rx) = mpsc::channel(1024);

    let this = InfluxDbUdp {
        address: address.to_string(),
        conn: None,
        // empty ipv6 udp package needs 48 bytes, subtract 50 for safety
        mtu: mtu.unwrap_or(1500) - 50,
        data: String::new(),
        channel: rx,
    };

    let join_handle = Some(tokio::spawn(async { this.finish().await }));

    Metrics {
        join_handle,
        channel: Some(tx),
    }
}

impl InfluxDbUdp {
    async fn add_data(&mut self, data: Arc<MetricsData>) -> Result<(), Error> {
        let new_data = utils::format_influxdb_line(&data)?;

        if self.data.len() + new_data.len() >= (self.mtu as usize) {
            self.flush().await?;
        }

        self.data.push_str(&new_data);

        if self.data.len() >= (self.mtu as usize) {
            self.flush().await?;
        }

        Ok(())
    }

    async fn flush(&mut self) -> Result<(), Error> {
        let conn = match self.conn.take() {
            Some(conn) => conn,
            None => udp::connect(&self.address).await?,
        };

        conn.send(self.data.split_off(0).as_bytes()).await?;
        self.conn = Some(conn);
        Ok(())
    }

    async fn finish(mut self) -> Result<(), Error> {
        while let Some(data) = self.channel.recv().await {
            self.add_data(data).await?;
        }

        self.flush().await?;

        Ok(())
    }
}
