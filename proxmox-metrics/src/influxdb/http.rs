use std::sync::Arc;

use anyhow::{bail, Error};
use hyper::Body;
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use tokio::sync::mpsc;

use proxmox_http::client::{SimpleHttp, SimpleHttpOptions};

use crate::influxdb::utils;
use crate::{Metrics, MetricsData};

struct InfluxDbHttp {
    client: SimpleHttp,
    healthuri: http::Uri,
    writeuri: http::Uri,
    token: Option<String>,
    max_body_size: usize,
    data: String,
    channel: mpsc::Receiver<Arc<MetricsData>>,
}

/// Tests the connection to the given influxdb http server with the given
/// parameters.
pub async fn test_influxdb_http(
    uri: &str,
    organization: &str,
    bucket: &str,
    token: Option<&str>,
    verify_tls: bool,
) -> Result<(), Error> {
    let (_tx, rx) = mpsc::channel(1);

    let this = InfluxDbHttp::new(uri, organization, bucket, token, verify_tls, 1, rx)?;

    this.test_connection().await
}

/// Get a [`Metrics`] handle for an influxdb server accessed via HTTPS.
pub fn influxdb_http(
    uri: &str,
    organization: &str,
    bucket: &str,
    token: Option<&str>,
    verify_tls: bool,
    max_body_size: usize,
) -> Result<Metrics, Error> {
    let (tx, rx) = mpsc::channel(1024);

    let this = InfluxDbHttp::new(
        uri,
        organization,
        bucket,
        token,
        verify_tls,
        max_body_size,
        rx,
    )?;

    let join_handle = Some(tokio::spawn(this.finish()));

    Ok(Metrics {
        join_handle,
        channel: Some(tx),
    })
}

impl InfluxDbHttp {
    fn new(
        uri: &str,
        organization: &str,
        bucket: &str,
        token: Option<&str>,
        verify_tls: bool,
        max_body_size: usize,
        channel: mpsc::Receiver<Arc<MetricsData>>,
    ) -> Result<Self, Error> {
        let client = if verify_tls {
            SimpleHttp::with_options(SimpleHttpOptions::default())
        } else {
            let mut ssl_connector = SslConnector::builder(SslMethod::tls()).unwrap();
            ssl_connector.set_verify(SslVerifyMode::NONE);
            SimpleHttp::with_ssl_connector(ssl_connector.build(), SimpleHttpOptions::default())
        };

        let uri: http::uri::Uri = uri.parse()?;
        let uri_parts = uri.into_parts();

        let base_path = if let Some(ref p) = uri_parts.path_and_query {
            p.path().trim_end_matches('/')
        } else {
            ""
        };

        let writeuri = http::uri::Builder::new()
            .scheme(uri_parts.scheme.clone().unwrap())
            .authority(uri_parts.authority.clone().unwrap())
            .path_and_query(format!(
                "{}/api/v2/write?org={}&bucket={}",
                base_path, organization, bucket
            ))
            .build()?;

        let healthuri = http::uri::Builder::new()
            .scheme(uri_parts.scheme.unwrap())
            .authority(uri_parts.authority.unwrap())
            .path_and_query(format!("{}/health", base_path))
            .build()?;

        Ok(InfluxDbHttp {
            client,
            writeuri,
            healthuri,
            token: token.map(String::from),
            max_body_size,
            data: String::new(),
            channel,
        })
    }

    async fn test_connection(&self) -> Result<(), Error> {
        let mut request = http::Request::builder().method("GET").uri(&self.healthuri);

        if let Some(token) = &self.token {
            request = request.header("Authorization", format!("Token {}", token));
        }

        let res = self.client.request(request.body(Body::empty())?).await?;

        let status = res.status();
        if !status.is_success() {
            bail!("got bad status: {}", status);
        }

        Ok(())
    }

    async fn add_data(&mut self, data: Arc<MetricsData>) -> Result<(), Error> {
        let new_data = utils::format_influxdb_line(&data)?;

        if self.data.len() + new_data.len() >= self.max_body_size {
            self.flush().await?;
        }

        self.data.push_str(&new_data);

        if self.data.len() >= self.max_body_size {
            self.flush().await?;
        }

        Ok(())
    }

    async fn flush(&mut self) -> Result<(), Error> {
        if self.data.is_empty() {
            return Ok(());
        }
        let mut request = http::Request::builder().method("POST").uri(&self.writeuri);

        if let Some(token) = &self.token {
            request = request.header("Authorization", format!("Token {}", token));
        }

        let request = request.body(Body::from(self.data.split_off(0)))?;

        let res = self.client.request(request).await?;

        let status = res.status();
        if !status.is_success() {
            bail!("got bad status: {}", status);
        }
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
