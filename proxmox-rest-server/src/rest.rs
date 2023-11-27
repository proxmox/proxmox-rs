use std::collections::HashMap;
use std::future::Future;
use std::hash::BuildHasher;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use anyhow::{bail, format_err, Error};
use futures::future::{FutureExt, TryFutureExt};
use futures::stream::TryStreamExt;
use hyper::body::HttpBody;
use hyper::header::{self, HeaderMap};
use hyper::http::request::Parts;
use hyper::{Body, Request, Response, StatusCode};
use lazy_static::lazy_static;
use regex::Regex;
use serde_json::Value;
use tokio::fs::File;
use tokio::time::Instant;
use tower_service::Service;
use url::form_urlencoded;

use proxmox_router::{
    check_api_permission, ApiHandler, ApiMethod, HttpError, Permission, RpcEnvironment,
    RpcEnvironmentType, UserInformation,
};
use proxmox_router::{http_bail, http_err};
use proxmox_schema::{ObjectSchemaType, ParameterSchema};

use proxmox_async::stream::AsyncReaderStream;
use proxmox_compression::{DeflateEncoder, Level};

use crate::{
    formatter::*, normalize_path, ApiConfig, AuthError, CompressionMethod, FileLogger,
    RestEnvironment,
};

extern "C" {
    fn tzset();
}

struct AuthStringExtension(String);

pub(crate) struct EmptyUserInformation {}

impl UserInformation for EmptyUserInformation {
    fn is_superuser(&self, _userid: &str) -> bool {
        false
    }
    fn is_group_member(&self, _userid: &str, _group: &str) -> bool {
        false
    }
    fn lookup_privs(&self, _userid: &str, _path: &[&str]) -> u64 {
        0
    }
}

/// REST server implementation (configured with [ApiConfig])
///
/// This struct implements the [Service] trait in order to use it with
/// [hyper::server::Builder::serve].
pub struct RestServer {
    api_config: Arc<ApiConfig>,
}

const MAX_URI_QUERY_LENGTH: usize = 3072;
const CHUNK_SIZE_LIMIT: u64 = 32 * 1024;

impl RestServer {
    /// Creates a new instance.
    pub fn new(api_config: ApiConfig) -> Self {
        Self {
            api_config: Arc::new(api_config),
        }
    }
}

impl<T: PeerAddress> Service<&T> for RestServer {
    type Response = ApiService;
    type Error = Error;
    type Future = std::future::Ready<Result<ApiService, Error>>;

    fn poll_ready(&mut self, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, ctx: &T) -> Self::Future {
        std::future::ready(match ctx.peer_addr() {
            Err(err) => Err(format_err!("unable to get peer address - {}", err)),
            Ok(peer) => Ok(ApiService {
                peer,
                api_config: Arc::clone(&self.api_config),
            }),
        })
    }
}

pub struct Redirector;

impl Default for Redirector {
    fn default() -> Self {
        Redirector::new()
    }
}

impl Redirector {
    pub fn new() -> Self {
        Self {}
    }
}

impl<T> Service<&T> for Redirector {
    type Response = RedirectService;
    type Error = Error;
    type Future = std::future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _ctx: &T) -> Self::Future {
        std::future::ready(Ok(RedirectService {}))
    }
}

pub struct RedirectService;

impl Service<Request<Body>> for RedirectService {
    type Response = Response<Body>;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let future = async move {
            let header_host_value = req
                .headers()
                .get("host")
                .and_then(|value| value.to_str().ok());

            let response = if let Some(value) = header_host_value {
                let location_value = String::from_iter(["https://", value]);

                let status_code = if matches!(*req.method(), http::Method::GET | http::Method::HEAD)
                {
                    StatusCode::MOVED_PERMANENTLY
                } else {
                    StatusCode::PERMANENT_REDIRECT
                };

                Response::builder()
                    .status(status_code)
                    .header("Location", String::from(location_value))
                    .body(Body::empty())?
            } else {
                Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::empty())?
            };

            Ok(response)
        };

        future.boxed()
    }
}

pub trait PeerAddress {
    fn peer_addr(&self) -> Result<std::net::SocketAddr, Error>;
}

// tokio_openssl's SslStream requires the stream to be pinned in order to accept it, and we need to
// accept before the peer address is requested, so let's just generally implement this for
// Pin<Box<T>>
impl<T: PeerAddress> PeerAddress for Pin<Box<T>> {
    fn peer_addr(&self) -> Result<std::net::SocketAddr, Error> {
        T::peer_addr(&**self)
    }
}

impl<T: PeerAddress> PeerAddress for tokio_openssl::SslStream<T> {
    fn peer_addr(&self) -> Result<std::net::SocketAddr, Error> {
        self.get_ref().peer_addr()
    }
}

impl PeerAddress for tokio::net::TcpStream {
    fn peer_addr(&self) -> Result<std::net::SocketAddr, Error> {
        Ok(self.peer_addr()?)
    }
}

impl PeerAddress for hyper::server::conn::AddrStream {
    fn peer_addr(&self) -> Result<std::net::SocketAddr, Error> {
        Ok(self.remote_addr())
    }
}

impl PeerAddress for tokio::net::UnixStream {
    fn peer_addr(&self) -> Result<std::net::SocketAddr, Error> {
        // TODO: Find a way to actually represent the vsock peer in the ApiService struct - for now
        // it doesn't really matter, so just use a fake IP address
        Ok(([0, 0, 0, 0], 807).into())
    }
}

#[cfg(feature = "rate-limited-stream")]
impl<T: PeerAddress> PeerAddress for proxmox_http::RateLimitedStream<T> {
    fn peer_addr(&self) -> Result<std::net::SocketAddr, Error> {
        self.inner().peer_addr()
    }
}

// Helper [Service] containing the peer Address
//
// The lower level connection [Service] implementation on
// [RestServer] extracts the peer address and return an [ApiService].
//
// Rust wants this type 'pub' here (else we get 'private type `ApiService`
// in public interface'). The type is still private because the crate does
// not export it.
pub struct ApiService {
    pub peer: std::net::SocketAddr,
    pub api_config: Arc<ApiConfig>,
}

fn log_response(
    logfile: Option<&Arc<Mutex<FileLogger>>>,
    peer: &std::net::SocketAddr,
    method: hyper::Method,
    path_query: &str,
    resp: &Response<Body>,
    user_agent: Option<String>,
) {
    if resp.extensions().get::<NoLogExtension>().is_some() {
        return;
    };

    // we also log URL-to-long requests, so avoid message bigger than PIPE_BUF (4k on Linux)
    // to profit from atomicty guarantees for O_APPEND opened logfiles
    let path = &path_query[..MAX_URI_QUERY_LENGTH.min(path_query.len())];

    let status = resp.status();
    if !(status.is_success() || status.is_informational()) {
        let reason = status.canonical_reason().unwrap_or("unknown reason");

        let message = match resp.extensions().get::<ErrorMessageExtension>() {
            Some(data) => &data.0,
            None => "request failed",
        };

        log::error!(
            "{} {}: {} {}: [client {}] {}",
            method.as_str(),
            path,
            status.as_str(),
            reason,
            peer,
            message
        );
    }
    if let Some(logfile) = logfile {
        let auth_id = match resp.extensions().get::<AuthStringExtension>() {
            Some(AuthStringExtension(auth_id)) => auth_id.clone(),
            None => "-".to_string(),
        };
        let now = proxmox_time::epoch_i64();
        // time format which apache/nginx use (by default), copied from pve-http-server
        let datetime = proxmox_time::strftime_local("%d/%m/%Y:%H:%M:%S %z", now)
            .unwrap_or_else(|_| "-".to_string());

        logfile.lock().unwrap().log(format!(
            "{} - {} [{}] \"{} {}\" {} {} {}",
            peer.ip(),
            auth_id,
            datetime,
            method.as_str(),
            path,
            status.as_str(),
            resp.body().size_hint().lower(),
            user_agent.unwrap_or_else(|| "-".to_string()),
        ));
    }
}

fn get_proxied_peer(headers: &HeaderMap) -> Option<std::net::SocketAddr> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r#"for="([^"]+)""#).unwrap();
    }
    let forwarded = headers.get(header::FORWARDED)?.to_str().ok()?;
    let capture = RE.captures(forwarded)?;
    let rhost = capture.get(1)?.as_str();

    rhost.parse().ok()
}

fn get_user_agent(headers: &HeaderMap) -> Option<String> {
    let agent = headers.get(header::USER_AGENT)?.to_str();
    agent
        .map(|s| {
            let mut s = s.to_owned();
            s.truncate(128);
            s
        })
        .ok()
}

impl Service<Request<Body>> for ApiService {
    type Response = Response<Body>;
    type Error = Error;
    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let path = req.uri().path_and_query().unwrap().as_str().to_owned();
        let method = req.method().clone();
        let user_agent = get_user_agent(req.headers());

        let config = Arc::clone(&self.api_config);
        let peer = match get_proxied_peer(req.headers()) {
            Some(proxied_peer) => proxied_peer,
            None => self.peer,
        };
        async move {
            let response = match Arc::clone(&config).handle_request(req, &peer).await {
                Ok(response) => response,
                Err(err) => {
                    let (err, code) = match err.downcast_ref::<HttpError>() {
                        Some(apierr) => (apierr.message.clone(), apierr.code),
                        _ => (err.to_string(), StatusCode::BAD_REQUEST),
                    };
                    Response::builder()
                        .status(code)
                        .extension(ErrorMessageExtension(err.to_string()))
                        .body(err.into())?
                }
            };
            let logger = config.get_access_log();
            log_response(logger, &peer, method, &path, &response, user_agent);
            Ok(response)
        }
        .boxed()
    }
}

fn parse_query_parameters<S: 'static + BuildHasher + Send>(
    param_schema: ParameterSchema,
    form: &str, // x-www-form-urlencoded body data
    parts: &Parts,
    uri_param: &HashMap<String, String, S>,
) -> Result<Value, Error> {
    let mut param_list: Vec<(String, String)> = vec![];

    if !form.is_empty() {
        for (k, v) in form_urlencoded::parse(form.as_bytes()).into_owned() {
            param_list.push((k, v));
        }
    }

    if let Some(query_str) = parts.uri.query() {
        for (k, v) in form_urlencoded::parse(query_str.as_bytes()).into_owned() {
            if k == "_dc" {
                continue;
            } // skip extjs "disable cache" parameter
            param_list.push((k, v));
        }
    }

    for (k, v) in uri_param {
        param_list.push((k.clone(), v.clone()));
    }

    let params = param_schema.parse_parameter_strings(&param_list, true)?;

    Ok(params)
}

async fn get_request_parameters<S: 'static + BuildHasher + Send>(
    param_schema: ParameterSchema,
    parts: Parts,
    req_body: Body,
    uri_param: HashMap<String, String, S>,
) -> Result<Value, Error> {
    let mut is_json = false;

    if let Some(value) = parts.headers.get(header::CONTENT_TYPE) {
        match value.to_str().map(|v| v.split(';').next()) {
            Ok(Some("application/x-www-form-urlencoded")) => {
                is_json = false;
            }
            Ok(Some("application/json")) => {
                is_json = true;
            }
            _ => bail!("unsupported content type {:?}", value.to_str()),
        }
    }

    let body = TryStreamExt::map_err(req_body, |err| {
        http_err!(BAD_REQUEST, "Problems reading request body: {}", err)
    })
    .try_fold(Vec::new(), |mut acc, chunk| async move {
        // FIXME: max request body size?
        if acc.len() + chunk.len() < 64 * 1024 {
            acc.extend_from_slice(&chunk);
            Ok(acc)
        } else {
            Err(http_err!(BAD_REQUEST, "Request body too large"))
        }
    })
    .await?;

    let utf8_data =
        std::str::from_utf8(&body).map_err(|err| format_err!("Request body not uft8: {}", err))?;

    if is_json {
        // treat empty body as empty paramater hash
        let mut params: Value = if utf8_data.is_empty() {
            Value::Object(serde_json::Map::new())
        } else {
            serde_json::from_str(utf8_data)?
        };
        for (k, v) in uri_param {
            if let Some((_optional, prop_schema)) = param_schema.lookup(&k) {
                params[&k] = prop_schema.parse_simple_value(&v)?;
            }
        }
        param_schema.verify_json(&params)?;
        Ok(params)
    } else {
        parse_query_parameters(param_schema, utf8_data, &parts, &uri_param)
    }
}

struct NoLogExtension();

async fn proxy_protected_request(
    info: &'static ApiMethod,
    mut parts: Parts,
    req_body: Body,
    peer: &std::net::SocketAddr,
) -> Result<Response<Body>, Error> {
    let mut uri_parts = parts.uri.clone().into_parts();

    uri_parts.scheme = Some(http::uri::Scheme::HTTP);
    uri_parts.authority = Some(http::uri::Authority::from_static("127.0.0.1:82"));
    let new_uri = http::Uri::from_parts(uri_parts).unwrap();

    parts.uri = new_uri;

    let mut request = Request::from_parts(parts, req_body);
    request.headers_mut().insert(
        header::FORWARDED,
        format!("for=\"{}\";", peer).parse().unwrap(),
    );

    let reload_timezone = info.reload_timezone;

    let resp = hyper::client::Client::new()
        .request(request)
        .map_err(Error::from)
        .map_ok(|mut resp| {
            resp.extensions_mut().insert(NoLogExtension());
            resp
        })
        .await?;

    if reload_timezone {
        unsafe {
            tzset();
        }
    }

    Ok(resp)
}

fn delay_unauth_time() -> std::time::Instant {
    std::time::Instant::now() + std::time::Duration::from_millis(3000)
}

fn access_forbidden_time() -> std::time::Instant {
    std::time::Instant::now() + std::time::Duration::from_millis(500)
}

pub(crate) async fn handle_api_request<Env: RpcEnvironment, S: 'static + BuildHasher + Send>(
    mut rpcenv: Env,
    info: &'static ApiMethod,
    formatter: &'static dyn OutputFormatter,
    parts: Parts,
    req_body: Body,
    uri_param: HashMap<String, String, S>,
) -> Result<Response<Body>, Error> {
    let compression = extract_compression_method(&parts.headers);

    let result = match info.handler {
        ApiHandler::AsyncHttp(handler) => {
            let params = parse_query_parameters(info.parameters, "", &parts, &uri_param)?;
            (handler)(parts, req_body, params, info, Box::new(rpcenv)).await
        }
        ApiHandler::StreamingSync(handler) => {
            let params =
                get_request_parameters(info.parameters, parts, req_body, uri_param).await?;
            (handler)(params, info, &mut rpcenv)
                .and_then(|data| formatter.format_data_streaming(data, &rpcenv))
        }
        ApiHandler::StreamingAsync(handler) => {
            let params =
                get_request_parameters(info.parameters, parts, req_body, uri_param).await?;
            (handler)(params, info, &mut rpcenv)
                .await
                .and_then(|data| formatter.format_data_streaming(data, &rpcenv))
        }
        ApiHandler::Sync(handler) => {
            let params =
                get_request_parameters(info.parameters, parts, req_body, uri_param).await?;
            (handler)(params, info, &mut rpcenv).map(|data| formatter.format_data(data, &rpcenv))
        }
        ApiHandler::Async(handler) => {
            let params =
                get_request_parameters(info.parameters, parts, req_body, uri_param).await?;
            (handler)(params, info, &mut rpcenv)
                .await
                .map(|data| formatter.format_data(data, &rpcenv))
        }
        _ => {
            bail!("Unknown API handler type");
        }
    };

    let mut resp = match result {
        Ok(resp) => resp,
        Err(err) => {
            if let Some(httperr) = err.downcast_ref::<HttpError>() {
                if httperr.code == StatusCode::UNAUTHORIZED {
                    tokio::time::sleep_until(Instant::from_std(delay_unauth_time())).await;
                }
            }
            formatter.format_error(err)
        }
    };

    let resp = match compression {
        Some(CompressionMethod::Deflate) => {
            resp.headers_mut().insert(
                header::CONTENT_ENCODING,
                CompressionMethod::Deflate.content_encoding(),
            );
            resp.map(|body| {
                Body::wrap_stream(DeflateEncoder::with_quality(
                    TryStreamExt::map_err(body, |err| {
                        proxmox_lang::io_format_err!("error during compression: {}", err)
                    }),
                    Level::Default,
                ))
            })
        }
        None => resp,
    };

    if info.reload_timezone {
        unsafe {
            tzset();
        }
    }

    Ok(resp)
}

async fn handle_unformatted_api_request<Env: RpcEnvironment, S: 'static + BuildHasher + Send>(
    mut rpcenv: Env,
    info: &'static ApiMethod,
    parts: Parts,
    req_body: Body,
    uri_param: HashMap<String, String, S>,
) -> Result<Response<Body>, Error> {
    let compression = extract_compression_method(&parts.headers);

    fn to_json_response<Env: RpcEnvironment>(
        value: Value,
        env: &Env,
    ) -> Result<Response<Body>, Error> {
        if let Some(attr) = env.result_attrib().as_object() {
            if !attr.is_empty() {
                http_bail!(
                    INTERNAL_SERVER_ERROR,
                    "result attributes are no longer supported"
                );
            }
        }
        let value = serde_json::to_string(&value)?;
        Ok(Response::builder().status(200).body(value.into())?)
    }

    let result = match info.handler {
        ApiHandler::AsyncHttp(handler) => {
            let params = parse_query_parameters(info.parameters, "", &parts, &uri_param)?;
            (handler)(parts, req_body, params, info, Box::new(rpcenv)).await
        }
        ApiHandler::Sync(handler) => {
            let params =
                get_request_parameters(info.parameters, parts, req_body, uri_param).await?;
            (handler)(params, info, &mut rpcenv).and_then(|v| to_json_response(v, &rpcenv))
        }
        ApiHandler::Async(handler) => {
            let params =
                get_request_parameters(info.parameters, parts, req_body, uri_param).await?;
            (handler)(params, info, &mut rpcenv)
                .await
                .and_then(|v| to_json_response(v, &rpcenv))
        }
        ApiHandler::StreamingSync(_) => http_bail!(
            INTERNAL_SERVER_ERROR,
            "old-style streaming calls not supported"
        ),
        ApiHandler::StreamingAsync(_) => http_bail!(
            INTERNAL_SERVER_ERROR,
            "old-style streaming calls not supported"
        ),
        _ => {
            bail!("Unknown API handler type");
        }
    };

    let mut resp = match result {
        Ok(resp) => resp,
        Err(err) => {
            if let Some(httperr) = err.downcast_ref::<HttpError>() {
                if httperr.code == StatusCode::UNAUTHORIZED {
                    tokio::time::sleep_until(Instant::from_std(delay_unauth_time())).await;
                }
            }
            return Err(err);
        }
    };

    let resp = match compression {
        Some(CompressionMethod::Deflate) => {
            resp.headers_mut().insert(
                header::CONTENT_ENCODING,
                CompressionMethod::Deflate.content_encoding(),
            );
            resp.map(|body| {
                Body::wrap_stream(DeflateEncoder::with_quality(
                    TryStreamExt::map_err(body, |err| {
                        proxmox_lang::io_format_err!("error during compression: {}", err)
                    }),
                    Level::Default,
                ))
            })
        }
        None => resp,
    };

    if info.reload_timezone {
        unsafe {
            tzset();
        }
    }

    Ok(resp)
}

fn extension_to_content_type(filename: &Path) -> (&'static str, bool) {
    if let Some(ext) = filename.extension().and_then(|osstr| osstr.to_str()) {
        return match ext {
            "css" => ("text/css", false),
            "html" => ("text/html", false),
            "js" => ("application/javascript", false),
            "json" => ("application/json", false),
            "map" => ("application/json", false),
            "png" => ("image/png", true),
            "ico" => ("image/x-icon", true),
            "gif" => ("image/gif", true),
            "svg" => ("image/svg+xml", false),
            "jar" => ("application/java-archive", true),
            "woff" => ("application/font-woff", true),
            "woff2" => ("application/font-woff2", true),
            "ttf" => ("application/font-snft", true),
            "pdf" => ("application/pdf", true),
            "epub" => ("application/epub+zip", true),
            "mp3" => ("audio/mpeg", true),
            "oga" => ("audio/ogg", true),
            "tgz" => ("application/x-compressed-tar", true),
            "wasm" => ("application/wasm", true),
            _ => ("application/octet-stream", false),
        };
    }

    ("application/octet-stream", false)
}

async fn simple_static_file_download(
    mut file: File,
    content_type: &'static str,
    compression: Option<CompressionMethod>,
) -> Result<Response<Body>, Error> {
    use tokio::io::AsyncReadExt;

    let mut data: Vec<u8> = Vec::new();

    let mut response = match compression {
        Some(CompressionMethod::Deflate) => {
            let mut enc = DeflateEncoder::with_quality(data, Level::Default);
            enc.compress_vec(&mut file, CHUNK_SIZE_LIMIT as usize)
                .await?;
            let mut response = Response::new(enc.into_inner().into());
            response.headers_mut().insert(
                header::CONTENT_ENCODING,
                CompressionMethod::Deflate.content_encoding(),
            );
            response
        }
        None => {
            file.read_to_end(&mut data)
                .await
                .map_err(|err| http_err!(BAD_REQUEST, "File read failed: {}", err))?;
            Response::new(data.into())
        }
    };

    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static(content_type),
    );

    Ok(response)
}

async fn chunked_static_file_download(
    file: File,
    content_type: &'static str,
    compression: Option<CompressionMethod>,
) -> Result<Response<Body>, Error> {
    let mut resp = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type);

    let body = match compression {
        Some(CompressionMethod::Deflate) => {
            resp = resp.header(
                header::CONTENT_ENCODING,
                CompressionMethod::Deflate.content_encoding(),
            );
            Body::wrap_stream(DeflateEncoder::with_quality(
                AsyncReaderStream::new(file),
                Level::Default,
            ))
        }
        None => Body::wrap_stream(AsyncReaderStream::new(file)),
    };

    Ok(resp.body(body).unwrap())
}

async fn handle_static_file_download(
    components: &[&str],
    filename: PathBuf,
    compression: Option<CompressionMethod>,
) -> Result<Response<Body>, Error> {
    let metadata = match tokio::fs::metadata(filename.clone()).await {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            http_bail!(NOT_FOUND, "no such file: '{}'", components.join("/"))
        }
        Err(err) => http_bail!(
            BAD_REQUEST,
            "File access problem on '{}': {}",
            components.join("/"),
            err.kind()
        ),
    };

    let (content_type, nocomp) = extension_to_content_type(&filename);
    let compression = if nocomp { None } else { compression };

    let file = File::open(filename).await.map_err(|err| {
        http_err!(
            BAD_REQUEST,
            "File open failed for '{}': {}",
            components.join("/"),
            err.kind()
        )
    })?;

    if metadata.len() < CHUNK_SIZE_LIMIT {
        simple_static_file_download(file, content_type, compression).await
    } else {
        chunked_static_file_download(file, content_type, compression).await
    }
}

// FIXME: support handling multiple compression methods
fn extract_compression_method(headers: &http::HeaderMap) -> Option<CompressionMethod> {
    if let Some(Ok(encodings)) = headers.get(header::ACCEPT_ENCODING).map(|v| v.to_str()) {
        for encoding in encodings.split(&[',', ' '][..]) {
            if let Ok(method) = encoding.parse() {
                return Some(method);
            }
        }
    }
    None
}

impl ApiConfig {
    pub async fn handle_request(
        self: Arc<ApiConfig>,
        req: Request<Body>,
        peer: &std::net::SocketAddr,
    ) -> Result<Response<Body>, Error> {
        let (parts, body) = req.into_parts();
        let method = parts.method.clone();
        let path = normalize_path(parts.uri.path())?;
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        let query = parts.uri.query().unwrap_or_default();
        if path.len() + query.len() > MAX_URI_QUERY_LENGTH {
            return Ok(Response::builder()
                .status(StatusCode::URI_TOO_LONG)
                .body("".into())
                .unwrap());
        }

        let env_type = self.env_type();
        let mut rpcenv = RestEnvironment::new(env_type, Arc::clone(&self));

        rpcenv.set_client_ip(Some(*peer));

        if let Some(handler) = self.find_handler(&components) {
            let relative_path_components = &components[handler.prefix.len()..];
            return handler
                .handle_request(ApiRequestData {
                    parts,
                    body,
                    peer,
                    config: &self,
                    full_path: &path,
                    relative_path_components,
                    rpcenv,
                })
                .await;
        }

        if method != hyper::Method::GET {
            http_bail!(BAD_REQUEST, "invalid http method for path");
        }

        if components.is_empty() {
            match self.check_auth(&parts.headers, &method).await {
                Ok((auth_id, _user_info)) => {
                    rpcenv.set_auth_id(Some(auth_id));
                    return Ok(self.get_index(rpcenv, parts).await);
                }
                Err(AuthError::Generic(_)) => {
                    tokio::time::sleep_until(Instant::from_std(delay_unauth_time())).await;
                }
                Err(AuthError::NoData) => {}
            }
            Ok(self.get_index(rpcenv, parts).await)
        } else {
            let filename = self.find_alias(&components);
            let compression = extract_compression_method(&parts.headers);
            handle_static_file_download(&components, filename, compression).await
        }
    }
}

pub(crate) struct Handler {
    pub prefix: &'static [&'static str],
    action: Action,
}

impl Handler {
    async fn handle_request(&self, data: ApiRequestData<'_>) -> Result<Response<Body>, Error> {
        self.action.handle_request(data).await
    }

    pub(crate) fn default_api2_handler(router: &'static proxmox_router::Router) -> Self {
        Self::formatted_router(&["api2"], router)
    }

    pub(crate) fn formatted_router(
        prefix: &'static [&'static str],
        router: &'static proxmox_router::Router,
    ) -> Self {
        Self {
            prefix,
            action: Action::Formatted(Formatted { router }),
        }
    }

    pub(crate) fn unformatted_router(
        prefix: &'static [&'static str],
        router: &'static proxmox_router::Router,
    ) -> Self {
        Self {
            prefix,
            action: Action::Unformatted(Unformatted { router }),
        }
    }
}

pub(crate) enum Action {
    Formatted(Formatted),
    Unformatted(Unformatted),
}

impl Action {
    async fn handle_request(&self, data: ApiRequestData<'_>) -> Result<Response<Body>, Error> {
        match self {
            Action::Formatted(a) => a.handle_request(data).await,
            Action::Unformatted(a) => a.handle_request(data).await,
        }
    }
}

pub struct ApiRequestData<'a> {
    parts: Parts,
    body: Body,
    peer: &'a std::net::SocketAddr,
    config: &'a ApiConfig,
    full_path: &'a str,
    relative_path_components: &'a [&'a str],
    rpcenv: RestEnvironment,
}

pub(crate) struct Formatted {
    router: &'static proxmox_router::Router,
}

impl Formatted {
    pub async fn handle_request(
        &self,
        ApiRequestData {
            parts,
            body,
            peer,
            config,
            full_path,
            relative_path_components,
            mut rpcenv,
        }: ApiRequestData<'_>,
    ) -> Result<Response<Body>, Error> {
        if relative_path_components.is_empty() {
            http_bail!(NOT_FOUND, "invalid api path '{}'", full_path);
        }

        let format = relative_path_components[0];

        let formatter: &dyn OutputFormatter = match format {
            "json" => JSON_FORMATTER,
            "extjs" => EXTJS_FORMATTER,
            _ => bail!("Unsupported output format '{}'.", format),
        };

        let mut uri_param = HashMap::new();
        let api_method = self.router.find_method(
            &relative_path_components[1..],
            parts.method.clone(),
            &mut uri_param,
        );

        let mut auth_required = true;
        if let Some(api_method) = api_method {
            if let Permission::World = *api_method.access.permission {
                auth_required = false; // no auth for endpoints with World permission
            }
        }

        let mut user_info: Box<dyn UserInformation + Send + Sync> =
            Box::new(EmptyUserInformation {});

        if auth_required {
            match config.check_auth(&parts.headers, &parts.method).await {
                Ok((authid, info)) => {
                    rpcenv.set_auth_id(Some(authid));
                    user_info = info;
                }
                Err(auth_err) => {
                    let err = match auth_err {
                        AuthError::Generic(err) => err,
                        AuthError::NoData => {
                            format_err!("no authentication credentials provided.")
                        }
                    };
                    // fixme: log Username??
                    rpcenv.log_failed_auth(None, &err.to_string());

                    // always delay unauthorized calls by 3 seconds (from start of request)
                    let err = http_err!(UNAUTHORIZED, "authentication failed - {}", err);
                    tokio::time::sleep_until(Instant::from_std(delay_unauth_time())).await;
                    return Err(err);
                }
            }
        }

        match api_method {
            None => {
                let err = http_err!(NOT_FOUND, "Path '{}' not found.", full_path);
                Ok(formatter.format_error(err))
            }
            Some(api_method) => {
                let auth_id = rpcenv.get_auth_id();
                let user_info = user_info;

                if !check_api_permission(
                    api_method.access.permission,
                    auth_id.as_deref(),
                    &uri_param,
                    user_info.as_ref(),
                ) {
                    let err = http_err!(FORBIDDEN, "permission check failed");
                    tokio::time::sleep_until(Instant::from_std(access_forbidden_time())).await;
                    return Ok(formatter.format_error(err));
                }

                let result = if api_method.protected
                    && rpcenv.env_type == RpcEnvironmentType::PUBLIC
                {
                    proxy_protected_request(api_method, parts, body, peer).await
                } else {
                    handle_api_request(rpcenv, api_method, formatter, parts, body, uri_param).await
                };

                let mut response = match result {
                    Ok(resp) => resp,
                    Err(err) => formatter.format_error(err),
                };

                if let Some(auth_id) = auth_id {
                    response
                        .extensions_mut()
                        .insert(AuthStringExtension(auth_id));
                }

                Ok(response)
            }
        }
    }
}

pub(crate) struct Unformatted {
    router: &'static proxmox_router::Router,
}

impl Unformatted {
    pub async fn handle_request(
        &self,
        ApiRequestData {
            parts,
            body,
            peer,
            config,
            full_path,
            relative_path_components,
            mut rpcenv,
        }: ApiRequestData<'_>,
    ) -> Result<Response<Body>, Error> {
        if relative_path_components.is_empty() {
            http_bail!(NOT_FOUND, "invalid api path '{}'", full_path);
        }

        let mut uri_param = HashMap::new();
        let api_method = self.router.find_method(
            relative_path_components,
            parts.method.clone(),
            &mut uri_param,
        );

        let mut auth_required = true;
        if let Some(api_method) = api_method {
            if let Permission::World = *api_method.access.permission {
                auth_required = false; // no auth for endpoints with World permission
            }
        }

        let user_info: Box<dyn UserInformation + Send + Sync>;

        if auth_required {
            match config.check_auth(&parts.headers, &parts.method).await {
                Ok((authid, info)) => {
                    rpcenv.set_auth_id(Some(authid));
                    user_info = info;
                }
                Err(auth_err) => {
                    let err = match auth_err {
                        AuthError::Generic(err) => err,
                        AuthError::NoData => {
                            format_err!("no authentication credentials provided.")
                        }
                    };
                    // fixme: log Username??
                    rpcenv.log_failed_auth(None, &err.to_string());

                    // always delay unauthorized calls by 3 seconds (from start of request)
                    let err = http_err!(UNAUTHORIZED, "authentication failed - {}", err);
                    tokio::time::sleep_until(Instant::from_std(delay_unauth_time())).await;
                    return Err(err);
                }
            }
        } else {
            user_info = Box::new(EmptyUserInformation {});
        }

        match api_method {
            None => http_bail!(NOT_FOUND, "Path '{}' not found.", full_path),
            Some(api_method) => {
                let auth_id = rpcenv.get_auth_id();
                let user_info = user_info;

                if !check_api_permission(
                    api_method.access.permission,
                    auth_id.as_deref(),
                    &uri_param,
                    user_info.as_ref(),
                ) {
                    let err = http_err!(FORBIDDEN, "permission check failed");
                    tokio::time::sleep_until(Instant::from_std(access_forbidden_time())).await;
                    return Err(err);
                }

                let result = if api_method.protected
                    && rpcenv.env_type == RpcEnvironmentType::PUBLIC
                {
                    proxy_protected_request(api_method, parts, body, peer).await
                } else {
                    handle_unformatted_api_request(rpcenv, api_method, parts, body, uri_param).await
                };

                let mut response = match result {
                    Ok(resp) => resp,
                    Err(err) => crate::formatter::error_to_response(err),
                };

                if let Some(auth_id) = auth_id {
                    response
                        .extensions_mut()
                        .insert(AuthStringExtension(auth_id));
                }

                Ok(response)
            }
        }
    }
}
