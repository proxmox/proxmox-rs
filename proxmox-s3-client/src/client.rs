use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, format_err, Context, Error};
use hyper::http::uri::Authority;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use openssl::hash::MessageDigest;
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use openssl::x509::X509StoreContextRef;
use tracing::error;

use proxmox_http::client::HttpsConnector;
use proxmox_http::{Body, RateLimiter};
use proxmox_schema::api_types::CERT_FINGERPRINT_SHA256_SCHEMA;

use crate::api_types::{S3ClientConfig, S3ClientSecretsConfig};

const S3_HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const S3_TCP_KEEPALIVE_TIME: u32 = 120;

/// Configuration options for client
pub struct S3ClientOptions {
    /// Endpoint to access S3 object store.
    pub endpoint: String,
    /// Port to access S3 object store.
    pub port: Option<u16>,
    /// Bucket to access S3 object store.
    pub bucket: String,
    /// Common prefix within bucket to use for objects keys for this client instance.
    pub common_prefix: String,
    /// Use path style bucket addressing over vhost style.
    pub path_style: bool,
    /// Secret key for S3 object store.
    pub secret_key: String,
    /// Access key for S3 object store.
    pub access_key: String,
    /// Region to access S3 object store.
    pub region: String,
    /// API certificate fingerprint for self signed certificates.
    pub fingerprint: Option<String>,
    /// Rate limit for put requests given as #reqest/s.
    pub put_rate_limit: Option<u64>,
}

impl S3ClientOptions {
    /// Construct options for the S3 client give the provided configuration parameters.
    pub fn from_config(
        config: S3ClientConfig,
        secrets: S3ClientSecretsConfig,
        bucket: String,
        common_prefix: String,
    ) -> Self {
        Self {
            endpoint: config.endpoint,
            port: config.port,
            bucket,
            common_prefix,
            path_style: config.path_style.unwrap_or_default(),
            region: config.region.unwrap_or("us-west-1".to_string()),
            fingerprint: config.fingerprint,
            access_key: config.access_key,
            secret_key: secrets.secret_key,
            put_rate_limit: config.put_rate_limit,
        }
    }
}

/// S3 client for object stores compatible with the AWS S3 API
pub struct S3Client {
    client: Client<HttpsConnector, Body>,
    options: S3ClientOptions,
    authority: Authority,
    put_rate_limiter: Option<Arc<Mutex<RateLimiter>>>,
}

impl S3Client {
    /// Creates a new S3 client instance, connecting to the provided endpoint using https given the
    /// provided options.
    pub fn new(options: S3ClientOptions) -> Result<Self, Error> {
        let expected_fingerprint = if let Some(ref fingerprint) = options.fingerprint {
            CERT_FINGERPRINT_SHA256_SCHEMA
                .unwrap_string_schema()
                .check_constraints(fingerprint)
                .context("invalid fingerprint provided")?;
            Some(fingerprint.to_lowercase())
        } else {
            None
        };
        let verified_fingerprint = Arc::new(Mutex::new(None));
        let trust_openssl_valid = Arc::new(Mutex::new(true));
        let mut ssl_connector_builder = SslConnector::builder(SslMethod::tls())?;
        ssl_connector_builder.set_verify_callback(
            SslVerifyMode::PEER,
            move |openssl_valid, context| match Self::verify_certificate_fingerprint(
                openssl_valid,
                context,
                expected_fingerprint.clone(),
                trust_openssl_valid.clone(),
            ) {
                Ok(None) => true,
                Ok(Some(fingerprint)) => {
                    *verified_fingerprint.lock().unwrap() = Some(fingerprint);
                    true
                }
                Err(err) => {
                    error!("certificate validation failed {err:#}");
                    false
                }
            },
        );

        let mut http_connector = HttpConnector::new();
        // want communication to object store backend api to always use https
        http_connector.enforce_http(false);
        http_connector.set_connect_timeout(Some(S3_HTTP_CONNECT_TIMEOUT));
        let https_connector = HttpsConnector::with_connector(
            http_connector,
            ssl_connector_builder.build(),
            S3_TCP_KEEPALIVE_TIME,
        );
        let client = Client::builder(TokioExecutor::new()).build::<_, Body>(https_connector);

        let authority_template = if let Some(port) = options.port {
            format!("{}:{port}", options.endpoint)
        } else {
            options.endpoint.clone()
        };
        let authority = authority_template
            .replace("{{bucket}}", &options.bucket)
            .replace("{{region}}", &options.region);
        let authority = Authority::try_from(authority)?;

        let put_rate_limiter = options.put_rate_limit.map(|limit| {
            let limiter = RateLimiter::new(limit, limit);
            Arc::new(Mutex::new(limiter))
        });

        Ok(Self {
            client,
            options,
            authority,
            put_rate_limiter,
        })
    }

    // TODO: replace with our shared TLS cert verification once available
    fn verify_certificate_fingerprint(
        openssl_valid: bool,
        context: &mut X509StoreContextRef,
        expected_fingerprint: Option<String>,
        trust_openssl: Arc<Mutex<bool>>,
    ) -> Result<Option<String>, Error> {
        let mut trust_openssl_valid = trust_openssl.lock().unwrap();

        // only rely on openssl prevalidation if was not forced earlier
        if openssl_valid && *trust_openssl_valid {
            return Ok(None);
        }

        let certificate = match context.current_cert() {
            Some(certificate) => certificate,
            None => bail!("context lacks current certificate."),
        };

        // force trust in case of a chain, but set flag to no longer trust prevalidation by openssl
        // see https://bugzilla.proxmox.com/show_bug.cgi?id=5248
        if context.error_depth() > 0 {
            *trust_openssl_valid = false;
            return Ok(None);
        }

        let certificate_digest = certificate
            .digest(MessageDigest::sha256())
            .context("failed to calculate certificate digest")?;
        let certificate_fingerprint = certificate_digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<String>>()
            .join(":");

        if let Some(expected_fingerprint) = expected_fingerprint {
            if expected_fingerprint == certificate_fingerprint {
                return Ok(Some(certificate_fingerprint));
            }
        }

        Err(format_err!(
            "unexpected certificate fingerprint {certificate_fingerprint}"
        ))
    }
}
