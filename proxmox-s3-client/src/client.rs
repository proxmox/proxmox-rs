use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, format_err, Context, Error};
use hyper::body::{Bytes, Incoming};
use hyper::http::method::Method;
use hyper::http::uri::{Authority, Parts, PathAndQuery, Scheme};
use hyper::http::{header, HeaderValue, StatusCode, Uri};
use hyper::{Request, Response};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use openssl::hash::MessageDigest;
use openssl::sha::Sha256;
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use openssl::x509::X509StoreContextRef;
use tracing::error;

use proxmox_http::client::HttpsConnector;
use proxmox_http::{Body, RateLimit, RateLimiter};
use proxmox_schema::api_types::CERT_FINGERPRINT_SHA256_SCHEMA;

use crate::api_types::{ProviderQuirks, S3ClientConfig};
use crate::aws_sign_v4::AWS_SIGN_V4_DATETIME_FORMAT;
use crate::aws_sign_v4::{aws_sign_v4_signature, aws_sign_v4_uri_encode};
use crate::object_key::S3ObjectKey;
use crate::response_reader::{
    CopyObjectResponse, DeleteObjectsResponse, GetObjectResponse, HeadObjectResponse,
    ListBucketsResponse, ListObjectsV2Response, PutObjectResponse, ResponseReader,
};

/// Default timeout for s3 api requests.
pub const S3_HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

const S3_HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const S3_TCP_KEEPALIVE_TIME: u32 = 120;
const MAX_S3_UPLOAD_RETRY: usize = 3;
// Assumed minimum upload rate of 1 KiB/s for dynamic put object request timeout calculation.
const S3_MIN_ASSUMED_UPLOAD_RATE: u64 = 1024;

/// S3 object key path prefix without the context prefix as defined by the client options.
///
/// The client option's context prefix will be pre-pended by the various client methods before
/// sending api requests.
pub enum S3PathPrefix {
    /// Path prefix relative to client's context prefix
    Some(String),
    /// No prefix
    None,
}

/// Configuration options for client
pub struct S3ClientOptions {
    /// Endpoint to access S3 object store.
    pub endpoint: String,
    /// Port to access S3 object store.
    pub port: Option<u16>,
    /// Bucket to access S3 object store.
    pub bucket: Option<String>,
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
    /// Provider implementation specific features and limitations
    pub provider_quirks: Vec<ProviderQuirks>,
}

impl S3ClientOptions {
    /// Construct options for the S3 client give the provided configuration parameters.
    pub fn from_config(
        config: S3ClientConfig,
        secret_key: String,
        bucket: Option<String>,
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
            secret_key,
            put_rate_limit: config.put_rate_limit,
            provider_quirks: config.provider_quirks.unwrap_or_default(),
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

        let authority = authority_template.replace("{{region}}", &options.region);

        let authority = if let Some(bucket) = &options.bucket {
            authority.replace("{{bucket}}", bucket)
        } else {
            authority.replace("{{bucket}}.", "")
        };

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

    /// Prepare API request by adding commonly required headers and perform request signing
    async fn prepare(&self, mut request: Request<Body>) -> Result<Request<Body>, Error> {
        let host_header = request
            .uri()
            .authority()
            .ok_or_else(|| format_err!("request missing authority"))?
            .to_string();

        // Content verification for aws s3 signature
        let mut hasher = Sha256::new();
        let contents = request
            .body()
            .as_bytes()
            .ok_or_else(|| format_err!("cannot prepare request with streaming body"))?;
        hasher.update(contents);
        // Use MD5 as upload integrity check, as other methods are not supported by all S3 object
        // store providers and might be ignored and this is recommended by AWS as described in
        // https://docs.aws.amazon.com/AmazonS3/latest/API/API_PutObject.html#API_PutObject_RequestSyntax
        let payload_md5 = md5::compute(contents);
        let payload_digest = hex::encode(hasher.finish());
        let payload_len = contents.len();

        let epoch = proxmox_time::epoch_i64();
        let datetime = proxmox_time::strftime_utc(AWS_SIGN_V4_DATETIME_FORMAT, epoch)?;

        request
            .headers_mut()
            .insert("x-amz-date", HeaderValue::from_str(&datetime)?);
        request
            .headers_mut()
            .insert("host", HeaderValue::from_str(&host_header)?);
        request.headers_mut().insert(
            "x-amz-content-sha256",
            HeaderValue::from_str(&payload_digest)?,
        );
        request.headers_mut().insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&payload_len.to_string())?,
        );
        if payload_len > 0 {
            let md5_digest = proxmox_base64::encode(*payload_md5);
            request
                .headers_mut()
                .insert("Content-MD5", HeaderValue::from_str(&md5_digest)?);
        }

        let signature = aws_sign_v4_signature(&request, &self.options, epoch, &payload_digest)?;

        request
            .headers_mut()
            .insert(header::AUTHORIZATION, HeaderValue::from_str(&signature)?);

        Ok(request)
    }

    /// Send API request to the configured endpoint using the inner https client.
    async fn send(
        &self,
        request: Request<Body>,
        timeout: Option<Duration>,
    ) -> Result<Response<Incoming>, Error> {
        let request = self.prepare(request).await?;
        if request.method() == Method::PUT {
            if let Some(limiter) = &self.put_rate_limiter {
                let sleep = {
                    let mut limiter = limiter.lock().unwrap();
                    limiter.register_traffic(Instant::now(), 1)
                };
                tokio::time::sleep(sleep).await;
            }
        }
        let response = if let Some(timeout) = timeout {
            tokio::time::timeout(timeout, self.client.request(request))
                .await
                .context("request timeout")??
        } else {
            self.client.request(request).await?
        };
        Ok(response)
    }

    /// Check if bucket exists and got permissions to access it.
    /// See reference docs: https://docs.aws.amazon.com/AmazonS3/latest/API/API_HeadBucket.html
    pub async fn head_bucket(&self) -> Result<(), Error> {
        let request = Request::builder()
            .method(Method::HEAD)
            .uri(self.build_uri("/", &[])?)
            .body(Body::empty())?;
        let response = self.send(request, Some(S3_HTTP_REQUEST_TIMEOUT)).await?;
        let (parts, _body) = response.into_parts();

        match parts.status {
            StatusCode::OK => (),
            StatusCode::BAD_REQUEST | StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => {
                bail!("bucket does not exist or no permission to access it")
            }
            status_code => bail!("unexpected status code {status_code}"),
        }

        Ok(())
    }

    /// List all buckets owned by the user authenticated via the access key.
    /// See reference docs: https://docs.aws.amazon.com/AmazonS3/latest/API/API_ListBuckets.html
    pub async fn list_buckets(&self) -> Result<ListBucketsResponse, Error> {
        let request = Request::builder()
            .method(Method::GET)
            .uri(self.build_uri("/", &[])?)
            .body(Body::empty())?;
        let response = self.send(request, Some(S3_HTTP_REQUEST_TIMEOUT)).await?;
        let response_reader = ResponseReader::new(response);
        response_reader.list_buckets_response().await
    }

    /// Fetch metadata from an object without returning the object itself.
    /// See reference docs: https://docs.aws.amazon.com/AmazonS3/latest/API/API_HeadObject.html
    pub async fn head_object(
        &self,
        object_key: S3ObjectKey,
    ) -> Result<Option<HeadObjectResponse>, Error> {
        let object_key = object_key.to_full_key(&self.options.common_prefix);
        let request = Request::builder()
            .method(Method::HEAD)
            .uri(self.build_uri(&object_key, &[])?)
            .body(Body::empty())?;
        let response = self.send(request, Some(S3_HTTP_REQUEST_TIMEOUT)).await?;
        let response_reader = ResponseReader::new(response);
        response_reader.head_object_response().await
    }

    /// Fetch an object from object store.
    /// See reference docs: https://docs.aws.amazon.com/AmazonS3/latest/API/API_GetObject.html
    pub async fn get_object(
        &self,
        object_key: S3ObjectKey,
    ) -> Result<Option<GetObjectResponse>, Error> {
        let object_key = object_key.to_full_key(&self.options.common_prefix);
        let request = Request::builder()
            .method(Method::GET)
            .uri(self.build_uri(&object_key, &[])?)
            .body(Body::empty())?;

        let response = self.send(request, Some(S3_HTTP_REQUEST_TIMEOUT)).await?;
        let response_reader = ResponseReader::new(response);
        response_reader.get_object_response().await
    }

    /// Returns some or all (up to 1,000) of the objects in a bucket with each request.
    /// See reference docs: https://docs.aws.amazon.com/AmazonS3/latest/API/API_GetObjectTagging.html
    pub async fn list_objects_v2(
        &self,
        prefix: &S3PathPrefix,
        continuation_token: Option<&str>,
    ) -> Result<ListObjectsV2Response, Error> {
        let mut query = vec![("list-type", "2")];
        let abs_prefix: String;
        if let S3PathPrefix::Some(prefix) = prefix {
            abs_prefix = if prefix.starts_with("/") {
                format!("{}{prefix}", self.options.common_prefix)
            } else {
                format!("{}/{prefix}", self.options.common_prefix)
            };
            query.push(("prefix", &abs_prefix));
        }
        if let Some(token) = continuation_token {
            query.push(("continuation-token", token));
        }
        let request = Request::builder()
            .method(Method::GET)
            .uri(self.build_uri("/", &query)?)
            .body(Body::empty())?;

        let response = self.send(request, Some(S3_HTTP_REQUEST_TIMEOUT)).await?;
        let response_reader = ResponseReader::new(response);
        response_reader.list_objects_v2_response().await
    }

    /// Add a new object to a bucket.
    ///
    /// Do not reupload if an object with matching key already exists in the bucket if the replace
    /// flag is not set.
    /// See reference docs: https://docs.aws.amazon.com/AmazonS3/latest/API/API_PutObject.html
    pub async fn put_object(
        &self,
        object_key: S3ObjectKey,
        object_data: Body,
        timeout: Option<Duration>,
        replace: bool,
    ) -> Result<PutObjectResponse, Error> {
        let object_key = object_key.to_full_key(&self.options.common_prefix);
        let mut request = Request::builder()
            .method(Method::PUT)
            .uri(self.build_uri(&object_key, &[])?)
            .header(header::CONTENT_TYPE, "binary/octet");

        if !replace {
            // Some providers not implement this and fails with error if the header is set,
            // see https://forum.proxmox.com/threads/168834/post-786278
            if !self
                .options
                .provider_quirks
                .contains(&ProviderQuirks::SkipIfNoneMatchHeader)
            {
                request = request.header(header::IF_NONE_MATCH, "*");
            }
        }

        let request = request.body(object_data)?;

        let response = self.send(request, timeout).await?;
        let response_reader = ResponseReader::new(response);
        response_reader.put_object_response().await
    }

    /// Removes an object from a bucket.
    /// See reference docs: https://docs.aws.amazon.com/AmazonS3/latest/API/API_DeleteObject.html
    pub async fn delete_object(&self, object_key: S3ObjectKey) -> Result<(), Error> {
        let object_key = object_key.to_full_key(&self.options.common_prefix);
        let request = Request::builder()
            .method(Method::DELETE)
            .uri(self.build_uri(&object_key, &[])?)
            .body(Body::empty())?;

        let response = self.send(request, None).await?;
        let response_reader = ResponseReader::new(response);
        response_reader.delete_object_response().await
    }

    /// Delete multiple objects from a bucket using a single HTTP request.
    /// See reference docs: https://docs.aws.amazon.com/AmazonS3/latest/API/API_DeleteObjects.html
    pub async fn delete_objects(
        &self,
        object_keys: &[S3ObjectKey],
    ) -> Result<DeleteObjectsResponse, Error> {
        if object_keys.is_empty() {
            return Ok(DeleteObjectsResponse::default());
        }

        let mut body = String::from(r#"<Delete xmlns="http://s3.amazonaws.com/doc/2006-03-01/">"#);
        for object_key in object_keys {
            body.push_str("<Object><Key>");
            body.push_str(object_key);
            body.push_str("</Key></Object>");
        }
        body.push_str("</Delete>");
        let request = Request::builder()
            .method(Method::POST)
            .uri(self.build_uri("/", &[("delete", "")])?)
            .body(Body::from(body))?;

        let response = self.send(request, Some(S3_HTTP_REQUEST_TIMEOUT)).await?;
        let response_reader = ResponseReader::new(response);
        response_reader.delete_objects_response().await
    }

    /// Creates a copy of an object that is already stored in Amazon S3.
    /// Uses the `x-amz-metadata-directive` set to `REPLACE`, therefore resulting in updated metadata.
    /// See reference docs: https://docs.aws.amazon.com/AmazonS3/latest/API/API_CopyObject.html
    pub async fn copy_object(
        &self,
        source_key: S3ObjectKey,
        destination_key: S3ObjectKey,
    ) -> Result<CopyObjectResponse, Error> {
        let bucket = match &self.options.bucket {
            Some(bucket) => bucket,
            None => bail!("missing bucket name for copy source"),
        };
        let copy_source = source_key.to_copy_source_key(bucket, &self.options.common_prefix);
        let copy_source = aws_sign_v4_uri_encode(&copy_source, true);
        let destination_key = destination_key.to_full_key(&self.options.common_prefix);
        let destination_key = aws_sign_v4_uri_encode(&destination_key, true);
        let request = Request::builder()
            .method(Method::PUT)
            .uri(self.build_uri(&destination_key, &[])?)
            .header("x-amz-copy-source", HeaderValue::from_str(&copy_source)?)
            .header(
                "x-amz-metadata-directive",
                HeaderValue::from_str("REPLACE")?,
            )
            .body(Body::empty())?;

        let response = self.send(request, Some(S3_HTTP_REQUEST_TIMEOUT)).await?;
        let response_reader = ResponseReader::new(response);
        response_reader.copy_object_response().await
    }

    /// Delete objects by given key prefix.
    /// Requires at least 2 api calls.
    pub async fn delete_objects_by_prefix(&self, prefix: &S3PathPrefix) -> Result<bool, Error> {
        // S3 API does not provide a convenient way to delete objects by key prefix.
        // List all objects with given group prefix and delete all objects found, so this
        // requires at least 2 API calls.
        let mut next_continuation_token: Option<String> = None;
        let mut delete_errors = false;
        loop {
            let list_objects_result = self
                .list_objects_v2(prefix, next_continuation_token.as_deref())
                .await?;

            let objects_to_delete: Vec<S3ObjectKey> = list_objects_result
                .contents
                .into_iter()
                .map(|item| item.key)
                .collect();

            let response = self.delete_objects(&objects_to_delete).await?;
            if response.error.is_some() {
                delete_errors = true;
            }

            if list_objects_result.is_truncated {
                next_continuation_token = list_objects_result
                    .next_continuation_token
                    .as_ref()
                    .cloned();
                continue;
            }
            break;
        }
        Ok(delete_errors)
    }

    /// Delete objects by given key prefix, but exclude items pre-filter based on suffix
    /// (including the parent component of the matched suffix). E.g. do not remove items in a
    /// snapshot directory, by matching based on the protected file marker (given as suffix).
    /// Items matching the suffix provided as `ignore` will be excluded in the parent of a matching
    /// suffix entry. E.g. owner and notes for a group, if a group snapshots was matched by a
    /// protected marker.
    ///
    /// Requires at least 2 api calls.
    pub async fn delete_objects_by_prefix_with_suffix_filter(
        &self,
        prefix: &S3PathPrefix,
        suffix: &str,
        excldue_from_parent: &[&str],
    ) -> Result<bool, Error> {
        // S3 API does not provide a convenient way to delete objects by key prefix.
        // List all objects with given group prefix and delete all objects found, so this
        // requires at least 2 API calls.
        let mut next_continuation_token: Option<String> = None;
        let mut delete_errors = false;
        let mut prefix_filters = Vec::new();
        let mut list_objects = Vec::new();
        loop {
            let list_objects_result = self
                .list_objects_v2(prefix, next_continuation_token.as_deref())
                .await?;

            let mut prefixes: Vec<String> = list_objects_result
                .contents
                .iter()
                .filter_map(|item| {
                    let prefix_filter = item.key.strip_suffix(suffix).map(|prefix| {
                        let path = Path::new(prefix);
                        if let Some(parent) = path.parent() {
                            for filter in excldue_from_parent {
                                let filter = parent.join(filter);
                                // valid utf-8 as combined from `str` values
                                prefix_filters.push(filter.to_string_lossy().to_string());
                            }
                        }
                        prefix.to_string()
                    });
                    if prefix_filter.is_none() {
                        list_objects.push(item.key.clone());
                    }
                    prefix_filter
                })
                .collect();
            prefix_filters.append(&mut prefixes);

            if list_objects_result.is_truncated {
                next_continuation_token = list_objects_result
                    .next_continuation_token
                    .as_ref()
                    .cloned();
                continue;
            }
            break;
        }

        let objects_to_delete: Vec<S3ObjectKey> = list_objects
            .into_iter()
            .filter_map(|item| {
                for prefix in &prefix_filters {
                    if item.strip_prefix(prefix).is_some() {
                        return None;
                    }
                }
                Some(item)
            })
            .collect();

        for objects in objects_to_delete.chunks(1000) {
            let result = self.delete_objects(objects).await?;
            if result.error.is_some() {
                delete_errors = true;
            }
        }

        Ok(delete_errors)
    }

    /// Upload the given object via the S3 api, not replacing it if already present in the object
    /// store.
    /// Retrying up to 3 times in case of error.
    #[inline(always)]
    pub async fn upload_no_replace_with_retry(
        &self,
        object_key: S3ObjectKey,
        object_data: Bytes,
    ) -> Result<bool, Error> {
        let replace = false;
        self.do_upload_with_retry(object_key, object_data, replace)
            .await
    }

    /// Upload the given object via the S3 api, replacing it if already present in the object store.
    /// Retrying up to 3 times in case of error.
    #[inline(always)]
    pub async fn upload_replace_with_retry(
        &self,
        object_key: S3ObjectKey,
        object_data: Bytes,
    ) -> Result<bool, Error> {
        let replace = true;
        self.do_upload_with_retry(object_key, object_data, replace)
            .await
    }

    /// Helper to perform the object upload and retry, wrapped by the corresponding methods
    /// to mask the `replace` flag.
    async fn do_upload_with_retry(
        &self,
        object_key: S3ObjectKey,
        object_data: Bytes,
        replace: bool,
    ) -> Result<bool, Error> {
        let content_size = object_data.len() as u64;
        let timeout_secs = content_size
            .div_ceil(S3_MIN_ASSUMED_UPLOAD_RATE)
            .max(S3_HTTP_REQUEST_TIMEOUT.as_secs());
        let timeout = Some(Duration::from_secs(timeout_secs));
        for retry in 0..MAX_S3_UPLOAD_RETRY {
            let body = Body::from(object_data.clone());
            match self
                .put_object(object_key.clone(), body, timeout, replace)
                .await
            {
                Ok(PutObjectResponse::Success(_response_body)) => return Ok(false),
                Ok(PutObjectResponse::PreconditionFailed) => return Ok(true),
                Ok(PutObjectResponse::NeedsRetry) => {
                    if retry >= MAX_S3_UPLOAD_RETRY - 1 {
                        bail!("concurrent operation, chunk upload failed")
                    }
                }
                Err(err) => {
                    if retry >= MAX_S3_UPLOAD_RETRY - 1 {
                        return Err(err.context("chunk upload failed"));
                    }
                }
            };
        }
        Ok(false)
    }

    #[inline(always)]
    /// Helper to generate [`Uri`] instance with common properties based on given path and query.
    fn build_uri(&self, mut path: &str, query: &[(&str, &str)]) -> Result<Uri, Error> {
        if path.starts_with('/') {
            path = &path[1..];
        }
        let path = aws_sign_v4_uri_encode(path, true);
        let mut path_and_query = if self.options.path_style {
            if let Some(bucket) = &self.options.bucket {
                format!("/{bucket}/{path}")
            } else {
                format!("/{path}")
            }
        } else {
            format!("/{path}")
        };

        if !query.is_empty() {
            path_and_query.push('?');
            // No further input validation as http::uri::Builder will check path and query
            let mut query_iter = query.iter().peekable();
            while let Some((key, value)) = query_iter.next() {
                let key = aws_sign_v4_uri_encode(key, false);
                path_and_query.push_str(&key);
                if !value.is_empty() {
                    let value = aws_sign_v4_uri_encode(value, false);
                    path_and_query.push('=');
                    path_and_query.push_str(&value);
                }
                if query_iter.peek().is_some() {
                    path_and_query.push('&');
                }
            }
        }

        let path_and_query =
            PathAndQuery::from_str(&path_and_query).context("failed to parse path and query")?;

        let mut uri_parts = Parts::default();
        uri_parts.scheme = Some(Scheme::HTTPS);
        uri_parts.authority = Some(self.authority.clone());
        uri_parts.path_and_query = Some(path_and_query);

        Uri::from_parts(uri_parts).context("failed to build uri")
    }
}
