use std::borrow::Cow;
use std::collections::HashMap;
use std::future::Future;
use std::hash::BuildHasher;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use anyhow::{Error, bail, format_err};
use futures::future::FutureExt;
use futures::stream::TryStreamExt;
use http_body_util::{BodyDataStream, BodyStream};
use hyper::body::{Body as HyperBody, Incoming};
use hyper::header::{self, HeaderMap};
use hyper::http::request::Parts;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn;
use hyper_util::server::graceful;
use hyper_util::service::TowerToHyperService;
use serde_json::Value;
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::Instant;
use tokio_stream::wrappers::ReceiverStream;
use tower_service::Service;
use url::form_urlencoded;

use proxmox_http::Body;
#[cfg(feature = "rate-limited-stream")]
use proxmox_http::{RateLimiterTag, RateLimiterTagsHandle};
#[cfg(not(feature = "rate-limited-stream"))]
type RateLimiterTagsHandle = ();
use proxmox_router::{
    ApiHandler, ApiMethod, HttpError, Permission, Router, RpcEnvironment, RpcEnvironmentType,
    UserInformation, check_api_permission,
};
use proxmox_router::{http_bail, http_err};
use proxmox_schema::{ObjectSchemaType, ParameterSchema};

use proxmox_async::stream::AsyncReaderStream;
use proxmox_compression::DeflateEncoder;
use proxmox_log::FileLogger;

use crate::{
    ApiConfig, AuthError, CompressionMethod, RestEnvironment, formatter::*, normalize_path,
};

unsafe extern "C" {
    fn tzset();
}

#[derive(Clone)]
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
const MAX_REQUEST_BODY_SIZE: usize = 512 * 1024;

const CHUNK_SIZE_LIMIT: u64 = 32 * 1024;

/// HTTP authority for forwarded requests; without a custom connector, it is also the destination.
const DEFAULT_PRIVILEGED_AUTHORITY: &str = "127.0.0.1:82";

impl RestServer {
    /// Creates a new instance.
    pub fn new(api_config: ApiConfig) -> Self {
        Self {
            api_config: Arc::new(api_config),
        }
    }

    #[cfg(not(feature = "rate-limited-stream"))]
    pub fn api_service<T>(&self, peer: &T) -> Result<ApiService, Error>
    where
        T: PeerAddress + ?Sized,
    {
        Ok(ApiService {
            peer: peer.peer_addr()?,
            api_config: Arc::clone(&self.api_config),
        })
    }

    #[cfg(feature = "rate-limited-stream")]
    pub fn api_service<T>(&self, peer: &T) -> Result<ApiService, Error>
    where
        T: PeerAddress + PeerRateLimitTags + ?Sized,
    {
        Ok(ApiService {
            peer: peer.peer_addr()?,
            api_config: Arc::clone(&self.api_config),
            rate_limit_tags: peer.rate_limiter_tag_handle(),
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

    pub fn redirect_service(&self) -> RedirectService {
        RedirectService {}
    }
}

#[derive(Clone)]
pub struct RedirectService;

impl RedirectService {
    pub async fn serve<S>(
        self,
        conn: S,
        mut graceful: Option<graceful::Watcher>,
    ) -> Result<(), Error>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let api_service = TowerToHyperService::new(self);
        let io = TokioIo::new(conn);
        let api_conn = conn::auto::Builder::new(TokioExecutor::new());
        let api_conn = api_conn.serve_connection_with_upgrades(io, api_service);
        if let Some(graceful) = graceful.take() {
            let api_conn = graceful.watch(api_conn);
            api_conn.await
        } else {
            api_conn.await
        }
        .map_err(|err| format_err!("error serving redirect connection: {err}"))
    }
}

impl Service<Request<Incoming>> for RedirectService {
    type Response = Response<Body>;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Incoming>) -> Self::Future {
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
                    .header("Location", location_value)
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

#[cfg(feature = "rate-limited-stream")]
pub trait PeerRateLimitTags {
    fn rate_limiter_tag_handle(&self) -> Option<RateLimiterTagsHandle>;
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

#[cfg(feature = "rate-limited-stream")]
impl<T: PeerRateLimitTags> PeerRateLimitTags for Pin<Box<T>> {
    fn rate_limiter_tag_handle(&self) -> Option<RateLimiterTagsHandle> {
        T::rate_limiter_tag_handle(&**self)
    }
}

#[cfg(feature = "rate-limited-stream")]
impl<T: PeerRateLimitTags> PeerRateLimitTags for tokio_openssl::SslStream<T> {
    fn rate_limiter_tag_handle(&self) -> Option<RateLimiterTagsHandle> {
        self.get_ref().rate_limiter_tag_handle()
    }
}

#[cfg(feature = "rate-limited-stream")]
impl PeerRateLimitTags for tokio::net::TcpStream {
    fn rate_limiter_tag_handle(&self) -> Option<RateLimiterTagsHandle> {
        None
    }
}

#[cfg(feature = "rate-limited-stream")]
impl PeerRateLimitTags for tokio::net::UnixStream {
    fn rate_limiter_tag_handle(&self) -> Option<RateLimiterTagsHandle> {
        None
    }
}

#[cfg(feature = "rate-limited-stream")]
impl<T> PeerRateLimitTags for proxmox_http::RateLimitedStream<T> {
    fn rate_limiter_tag_handle(&self) -> Option<RateLimiterTagsHandle> {
        self.rate_limiter_tags_handle().cloned()
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
#[derive(Clone)]
pub struct ApiService {
    pub peer: std::net::SocketAddr,
    pub api_config: Arc<ApiConfig>,
    #[cfg(feature = "rate-limited-stream")]
    pub rate_limit_tags: Option<RateLimiterTagsHandle>,
}

impl ApiService {
    pub async fn serve<S>(
        self,
        conn: S,
        mut graceful: Option<graceful::Watcher>,
    ) -> Result<(), Error>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let api_service = TowerToHyperService::new(self);
        let io = TokioIo::new(conn);
        let api_conn = conn::auto::Builder::new(TokioExecutor::new());
        let api_conn = api_conn.serve_connection_with_upgrades(io, api_service);
        if let Some(graceful) = graceful.take() {
            let api_conn = graceful.watch(api_conn);
            api_conn.await
        } else {
            api_conn.await
        }
        .map_err(|err| format_err!("error serving connection: {err}"))
    }
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
    if status.is_client_error() || status.is_server_error() {
        let reason = status.canonical_reason().unwrap_or("unknown reason");

        let message = match resp.extensions().get::<ErrorMessageExtension>() {
            Some(data) => &data.0,
            None => "request failed",
        };

        // A client error is the client's, like a request for a missing file or an expired
        // ticket, and says nothing about the health of the server.
        let level = if status.is_server_error() {
            log::Level::Error
        } else {
            log::Level::Warn
        };
        log::log!(
            level,
            "{method} {path}: {status} {reason}: [client {peer}] {message}",
            status = status.as_str(),
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
            "{ip} - {auth_id} [{datetime}] \"{method} {path}\" {status} {size} {user_agent}",
            ip = peer.ip(),
            status = status.as_str(),
            size = resp.body().size_hint().lower(),
            user_agent = user_agent.unwrap_or_else(|| "-".to_string()),
        ));
    }
}

fn get_proxied_peer(
    headers: &HeaderMap,
    config: &ApiConfig,
    peer: &std::net::SocketAddr,
) -> Option<std::net::SocketAddr> {
    let real_ip_header = config.real_ip_header.as_ref()?;

    if let Some(ref allow_from) = config.real_ip_allow_from {
        let peer_ip = peer.ip();
        if !allow_from
            .iter()
            .any(|cidr| cidr.contains_address(&peer_ip))
        {
            return None;
        }
    }

    if real_ip_header.eq_ignore_ascii_case("FORWARDED") {
        // multiple header lines are equivalent to one comma separated list, so the trusted
        // element is in the last of them
        let forwarded = headers
            .get_all(header::FORWARDED)
            .iter()
            .next_back()?
            .to_str()
            .ok()?;
        return parse_peer_address(&forwarded_for(forwarded)?);
    }

    let value = headers.get(real_ip_header.as_str())?.to_str().ok()?;
    parse_peer_address(value)
}

/// Extract the `for` parameter from the last element of an RFC 7239 `Forwarded` header.
///
/// Proxies append their own element, so the last one was added by the proxy we are talking to,
/// which is the only one that can be trusted; everything in front of it is controlled by whoever
/// talked to that proxy. Delimiters inside a quoted value separate nothing, so the header has to
/// be scanned rather than split.
fn forwarded_for(header: &str) -> Option<Cow<'_, str>> {
    let element = split_unquoted(header, ',')?.pop()?;

    for parameter in split_unquoted(element, ';')? {
        let Some((name, value)) = parameter.split_once('=') else {
            continue;
        };

        if name.trim().eq_ignore_ascii_case("for") {
            return Some(unquote(value.trim()));
        }
    }

    None
}

/// Split at every occurrence of `delimiter` that is not inside a quoted string.
///
/// Returns `None` if a quoted string or an escape is left open, since the delimiters behind it
/// cannot be placed: a peer that gets its value appended to could otherwise hide the delimiter in
/// front of the element a proxy adds and have its own element read as the last one.
fn split_unquoted(value: &str, delimiter: char) -> Option<Vec<&str>> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;

    for (index, c) in value.char_indices() {
        if escaped {
            escaped = false;
        } else if quoted && c == '\\' {
            escaped = true;
        } else if c == '"' {
            quoted = !quoted;
        } else if c == delimiter && !quoted {
            parts.push(&value[start..index]);
            start = index + c.len_utf8();
        }
    }
    if quoted || escaped {
        return None;
    }
    parts.push(&value[start..]);

    Some(parts)
}

/// Strip the quotes off a quoted string and resolve its escapes.
fn unquote(value: &str) -> Cow<'_, str> {
    let Some(inner) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        return Cow::Borrowed(value);
    };

    if !inner.contains('\\') {
        return Cow::Borrowed(inner);
    }

    let mut out = String::with_capacity(inner.len());
    let mut escaped = false;
    for c in inner.chars() {
        if escaped || c != '\\' {
            out.push(c);
            escaped = false;
        } else {
            escaped = true;
        }
    }

    Cow::Owned(out)
}

/// Parse a peer address, accepting `<ip>:<port>` as well as a bare, optionally bracketed, `<ip>`.
///
/// RFC 7239 leaves the port optional and requires brackets around IPv6 addresses, so a peer that
/// comes without a port is reported with port 0.
fn parse_peer_address(value: &str) -> Option<std::net::SocketAddr> {
    let value = value.trim();

    if let Ok(addr) = value.parse::<std::net::SocketAddr>() {
        return Some(addr);
    }

    let value = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(value);

    Some(std::net::SocketAddr::new(value.parse().ok()?, 0))
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

impl Service<Request<Incoming>> for ApiService {
    type Response = Response<Body>;
    type Error = Error;
    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Incoming>) -> Self::Future {
        let path = req
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str().to_owned())
            .unwrap_or_default();
        let method = req.method().clone();
        let user_agent = get_user_agent(req.headers());

        let config = Arc::clone(&self.api_config);
        let peer = match get_proxied_peer(req.headers(), &config, &self.peer) {
            Some(proxied_peer) => proxied_peer,
            None => self.peer,
        };
        #[cfg(feature = "rate-limited-stream")]
        let rate_limit_tags = self.rate_limit_tags.clone();
        #[cfg(not(feature = "rate-limited-stream"))]
        let rate_limit_tags: Option<RateLimiterTagsHandle> = None;

        let header = self.api_config
            .auth_cookie_name
            .as_ref()
            .map(|name|{
                let host_cookie = format!("{name}=; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Secure; SameSite=Lax; HttpOnly; Path=/;");

                // SAFETY: this can only fail if the cookie name is not valid in http headers.
                // since this is about an authentication cookie, this should never happen.
                hyper::header::HeaderValue::from_str(&host_cookie)
                    .expect("auth cookie name has characters that are not valid for http headers")
             });

        async move {
            #[cfg(feature = "rate-limited-stream")]
            if let Some(handle) = rate_limit_tags.as_ref() {
                handle.set_tags(Vec::new());
            }

            let mut response = match Arc::clone(&config)
                .handle_request(req, &peer, rate_limit_tags)
                .await
            {
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

            if let Some(cookie_header) = header {
                // remove auth cookies that javascript based clients can not unset
                if response.status() == StatusCode::UNAUTHORIZED {
                    response
                        .headers_mut()
                        .insert(hyper::header::SET_COOKIE, cookie_header);
                }
            }

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

async fn get_request_parameters(
    param_schema: ParameterSchema,
    parts: &Parts,
    req_body: Incoming,
    uri_param: HashMap<String, String>,
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

    let stream_body = BodyStream::new(req_body);
    let body = TryStreamExt::map_err(stream_body, |err| {
        http_err!(BAD_REQUEST, "Problems reading request body: {}", err)
    })
    .try_fold(Vec::new(), |mut acc, frame| async move {
        // FIXME: max request body size?
        let frame = frame
            .into_data()
            .map_err(|err| format_err!("Failed to read request body frame - {err:?}"))?;
        if acc.len() + frame.len() < MAX_REQUEST_BODY_SIZE {
            acc.extend_from_slice(&frame);
            Ok(acc)
        } else {
            Err(http_err!(BAD_REQUEST, "Request body too large"))
        }
    })
    .await?;

    let utf8_data =
        std::str::from_utf8(&body).map_err(|err| format_err!("Request body not uft8: {}", err))?;

    if is_json {
        // treat empty body as empty parameter hash
        let mut params: Value = if utf8_data.is_empty() {
            Value::Object(serde_json::Map::new())
        } else {
            serde_json::from_str(utf8_data)?
        };

        for (k, v) in &uri_param {
            let with_type_field = |key: &str, func: &mut dyn FnMut(Option<&str>)| {
                // Since uri_param gets merged into the params hash we consider them first, so the
                // type lookups happen as if the merge already happened.
                if let Some(value) = uri_param.get(key) {
                    func(Some(value));
                } else {
                    func(params[key].as_str());
                }
            };
            if let Some((_optional, prop_schema)) = param_schema.lookup_schema(&k, &with_type_field)
            {
                params[&k] = prop_schema.parse_simple_value(&v)?;
            }
        }
        // Rewrite deprecated alias keys to their canonical names before verification, so
        // macro-generated handlers see only canonical names.
        param_schema.canonicalize_aliases(&mut params)?;
        param_schema.verify_json(&params)?;
        Ok(params)
    } else {
        parse_query_parameters(param_schema, utf8_data, parts, &uri_param)
    }
}

#[derive(Clone)]
struct NoLogExtension();

async fn proxy_protected_request(
    config: &ApiConfig,
    info: &ApiMethod,
    mut parts: Parts,
    req_body: Incoming,
    peer: &std::net::SocketAddr,
) -> Result<Response<Body>, Error> {
    let mut uri_parts = parts.uri.clone().into_parts();

    uri_parts.scheme = Some(http::uri::Scheme::HTTP);
    uri_parts.authority = Some(http::uri::Authority::from_static(
        DEFAULT_PRIVILEGED_AUTHORITY,
    ));
    let new_uri = http::Uri::from_parts(uri_parts).unwrap();

    parts.uri = new_uri;

    let mut request = Request::from_parts(parts, req_body);
    request.headers_mut().insert(
        header::FORWARDED,
        format!("for=\"{peer}\";").parse().unwrap(),
    );

    let reload_timezone = info.reload_timezone;

    let mut resp = match config.privileged_addr.clone() {
        None => {
            hyper_util::client::legacy::Client::builder(TokioExecutor::new())
                .build_http()
                .request(request)
                .await?
        }
        Some(addr) => {
            hyper_util::client::legacy::Client::builder(TokioExecutor::new())
                .build(addr)
                .request(request)
                .await?
        }
    };
    resp.extensions_mut().insert(NoLogExtension());

    if reload_timezone {
        unsafe {
            tzset();
        }
    }

    Ok(resp.map(|b| Body::wrap_stream(BodyDataStream::new(b))))
}

/// Set the rate limiter tags for the current request, if the peer is rate limited at all.
#[cfg(feature = "rate-limited-stream")]
fn set_rate_limit_user(handle: Option<&RateLimiterTagsHandle>, auth_id: Option<&str>) {
    if let Some(handle) = handle {
        handle.set_tags(match auth_id {
            Some(auth_id) => vec![RateLimiterTag::User(auth_id.to_string())],
            None => Vec::new(),
        });
    }
}

#[cfg(not(feature = "rate-limited-stream"))]
fn set_rate_limit_user(_handle: Option<&RateLimiterTagsHandle>, _auth_id: Option<&str>) {}

/// Why a request got rejected before it could be dispatched to its API method.
///
/// The formatted and the unformatted endpoint render these differently, but everything leading up
/// to them has to stay identical, which is why the checks live in [`authorize_request`].
enum RequestError {
    /// Authentication is required for this method, but did not succeed.
    Unauthorized(Error),
    /// No API method is registered for this path and HTTP method.
    NotFound(Error),
    /// The API method exists, but the authenticated user must not call it.
    Forbidden(Error),
}

impl From<RequestError> for Error {
    fn from(err: RequestError) -> Self {
        match err {
            RequestError::Unauthorized(err)
            | RequestError::NotFound(err)
            | RequestError::Forbidden(err) => err,
        }
    }
}

/// Everything the shared authorization path needs besides the router and the path itself.
struct RequestContext<'a> {
    parts: &'a Parts,
    config: &'a ApiConfig,
    full_path: &'a str,
    rate_limit_tags: Option<&'a RateLimiterTagsHandle>,
    request_start: Instant,
}

/// Look up the API method for a request and run the authentication and permission checks.
///
/// On success the authenticated user is recorded in `rpcenv` and the path parameters collected
/// while routing are returned together with the method.
async fn authorize_request(
    router: &'static Router,
    path_components: &[&str],
    request: &RequestContext<'_>,
    rpcenv: &mut RestEnvironment,
) -> Result<(&'static ApiMethod, HashMap<String, String>), RequestError> {
    let request_start = request.request_start;
    let mut uri_param = HashMap::new();
    let api_method = router.find_method(
        path_components,
        request.parts.method.clone(),
        &mut uri_param,
    );

    // no auth for endpoints with World permission
    let auth_required = match api_method {
        Some(api_method) => !matches!(api_method.access.permission, Permission::World),
        None => true,
    };

    let user_info: Box<dyn UserInformation + Send + Sync> = if auth_required {
        match request
            .config
            .check_auth(&request.parts.headers, &request.parts.method)
            .await
        {
            Ok((authid, info)) => {
                set_rate_limit_user(request.rate_limit_tags, Some(&authid));
                rpcenv.set_auth_id(Some(authid));
                info
            }
            Err(auth_err) => {
                set_rate_limit_user(request.rate_limit_tags, None);
                let err = match auth_err {
                    AuthError::Generic(err) => err,
                    AuthError::NoData => format_err!("no authentication credentials provided."),
                };
                // fixme: log Username??
                rpcenv.log_failed_auth(None, &err.to_string());

                // always delay unauthorized calls by 3 seconds (from start of request)
                tokio::time::sleep_until(delay_unauth_time(request_start)).await;
                return Err(RequestError::Unauthorized(http_err!(
                    UNAUTHORIZED,
                    "authentication failed"
                )));
            }
        }
    } else {
        set_rate_limit_user(request.rate_limit_tags, None);
        Box::new(EmptyUserInformation {})
    };

    let Some(api_method) = api_method else {
        return Err(RequestError::NotFound(http_err!(
            NOT_FOUND,
            "Path '{}' not found.",
            request.full_path
        )));
    };

    if !check_api_permission(
        api_method.access.permission,
        rpcenv.get_auth_id().as_deref(),
        &uri_param,
        user_info.as_ref(),
    ) {
        tokio::time::sleep_until(access_forbidden_time(request_start)).await;
        return Err(RequestError::Forbidden(http_err!(
            FORBIDDEN,
            "permission check failed"
        )));
    }

    Ok((api_method, uri_param))
}

/// Mask authentication runtime up to three seconds, measured before the check starts.
fn delay_unauth_time(auth_start: Instant) -> Instant {
    auth_start + std::time::Duration::from_millis(3000)
}

/// When to answer a request that was denied by the permission check, see [`delay_unauth_time`].
fn access_forbidden_time(request_start: Instant) -> Instant {
    request_start + std::time::Duration::from_millis(500)
}

fn handle_stream_as_json_seq(stream: proxmox_router::Stream) -> Result<Response<Body>, Error> {
    let (send, body) = tokio::sync::mpsc::channel::<Result<Vec<u8>, Error>>(1);
    tokio::spawn(async move {
        use futures::StreamExt;

        let mut stream = stream.into_inner();
        while let Some(record) = stream.next().await {
            if send.send(Ok(record.to_bytes())).await.is_err() {
                break;
            }
        }
    });

    Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/json-seq")
        .body(Body::wrap_stream(ReceiverStream::new(body)))
        .map_err(Error::from)
}

fn handle_sync_stream_as_json_seq(
    iter: proxmox_router::SyncStream,
) -> Result<Response<Body>, Error> {
    let iter = iter
        .into_inner()
        .map(|record| Ok::<_, Error>(record.to_bytes()));

    Ok(Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/json-seq")
        .body(Body::wrap_stream(futures::stream::iter(iter)))?)
}

pub(crate) async fn handle_api_request<Env: RpcEnvironment>(
    mut rpcenv: Env,
    info: &'static ApiMethod,
    formatter: Option<&'static dyn OutputFormatter>,
    parts: Parts,
    req_body: Incoming,
    uri_param: HashMap<String, String>,
) -> Result<Response<Body>, Error> {
    let formatter = formatter.unwrap_or(crate::formatter::DIRECT_JSON_FORMATTER);

    let compression = extract_compression_method(&parts.headers);

    let accept_json_seq = parts.headers.get_all(http::header::ACCEPT).iter().any(|h| {
        h.as_ref()
            .split(|&b| b == b',')
            .map(|e| e.trim_ascii_start())
            .any(|e| e == b"application/json-seq" || e.starts_with(b"application/json-seq;"))
    });

    // Body reception is client-controlled and must not consume the delay that masks a handler's
    // authentication check. Start that deadline only once its parameters are available.
    let handler_start;
    let result = match info.handler {
        ApiHandler::AsyncHttp(handler) => {
            let params = parse_query_parameters(info.parameters, "", &parts, &uri_param)?;
            handler_start = Instant::now();
            (handler)(parts, req_body, params, info, Box::new(rpcenv)).await
        }
        ApiHandler::AsyncHttpBodyParameters(handler) => {
            let params =
                get_request_parameters(info.parameters, &parts, req_body, uri_param).await?;
            handler_start = Instant::now();
            (handler)(parts, params, info, Box::new(rpcenv)).await
        }
        ApiHandler::StreamSync(handler) => {
            let params =
                get_request_parameters(info.parameters, &parts, req_body, uri_param).await?;
            handler_start = Instant::now();
            match (handler)(params, info, &mut rpcenv) {
                Ok(iter) if accept_json_seq => handle_sync_stream_as_json_seq(iter),
                Ok(iter) => iter
                    .try_collect()
                    .map(|data| formatter.format_data(data, &rpcenv)),
                Err(err) => Err(err),
            }
        }
        ApiHandler::StreamAsync(handler) => {
            let params =
                get_request_parameters(info.parameters, &parts, req_body, uri_param).await?;
            handler_start = Instant::now();
            match (handler)(params, info, &mut rpcenv).await {
                Ok(stream) if accept_json_seq => handle_stream_as_json_seq(stream),
                Ok(stream) => stream
                    .try_collect()
                    .await
                    .map(|data| formatter.format_data(data, &rpcenv)),
                Err(err) => Err(err),
            }
        }
        ApiHandler::SerializingSync(handler) => {
            let params =
                get_request_parameters(info.parameters, &parts, req_body, uri_param).await?;
            handler_start = Instant::now();
            (handler)(params, info, &mut rpcenv)
                .and_then(|data| formatter.format_data_streaming(data, &rpcenv))
        }
        ApiHandler::SerializingAsync(handler) => {
            let params =
                get_request_parameters(info.parameters, &parts, req_body, uri_param).await?;
            handler_start = Instant::now();
            (handler)(params, info, &mut rpcenv)
                .await
                .and_then(|data| formatter.format_data_streaming(data, &rpcenv))
        }
        ApiHandler::Sync(handler) => {
            let params =
                get_request_parameters(info.parameters, &parts, req_body, uri_param).await?;
            handler_start = Instant::now();
            (handler)(params, info, &mut rpcenv).map(|data| formatter.format_data(data, &rpcenv))
        }
        ApiHandler::Async(handler) => {
            let params =
                get_request_parameters(info.parameters, &parts, req_body, uri_param).await?;
            handler_start = Instant::now();
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
                    tokio::time::sleep_until(delay_unauth_time(handler_start)).await;
                }
            }
            formatter.format_error(err)
        }
    };

    let is_streaming = accept_json_seq
        && resp
            .headers()
            .get(http::header::CONTENT_TYPE)
            .is_some_and(|h| h.as_ref().starts_with(b"application/json-seq"));

    let resp = match compression {
        Some(CompressionMethod::Deflate) => {
            resp.headers_mut().insert(
                header::CONTENT_ENCODING,
                CompressionMethod::Deflate.content_encoding(),
            );
            resp.map(|body| {
                Body::wrap_stream(
                    DeflateEncoder::builder(TryStreamExt::map_err(
                        BodyDataStream::new(body),
                        |err| proxmox_lang::io_format_err!("error during compression: {}", err),
                    ))
                    .zlib(true)
                    .flush_window(is_streaming.then_some(64 * 1024))
                    .build(),
                )
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
            let mut enc = DeflateEncoder::builder(data).zlib(true).build();
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
            Body::wrap_stream(
                DeflateEncoder::builder(AsyncReaderStream::new(file))
                    .zlib(true)
                    .build(),
            )
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
        for encoding in encodings.split(',') {
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
        req: Request<Incoming>,
        peer: &std::net::SocketAddr,
        rate_limit_tags: Option<RateLimiterTagsHandle>,
    ) -> Result<Response<Body>, Error> {
        let request_start = Instant::now();
        let (parts, body) = req.into_parts();
        let method = parts.method.clone();
        let path = normalize_path(parts.uri.path())?;
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        let query = parts.uri.query().unwrap_or_default();
        if path.len() + query.len() > MAX_URI_QUERY_LENGTH {
            return Ok(Response::builder()
                .status(StatusCode::URI_TOO_LONG)
                .body(Body::empty())
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
                    rate_limit_tags: rate_limit_tags.clone(),
                    request_start,
                })
                .await;
        }

        if method != hyper::Method::GET {
            http_bail!(BAD_REQUEST, "invalid http method for path");
        }

        if components.is_empty() {
            match self.check_auth(&parts.headers, &method).await {
                Ok((auth_id, _user_info)) => {
                    set_rate_limit_user(rate_limit_tags.as_ref(), Some(&auth_id));
                    rpcenv.set_auth_id(Some(auth_id));
                    return Ok(self.get_index(rpcenv, parts).await);
                }
                Err(AuthError::Generic(_)) => {
                    set_rate_limit_user(rate_limit_tags.as_ref(), None);
                    tokio::time::sleep_until(delay_unauth_time(request_start)).await;
                }
                Err(AuthError::NoData) => set_rate_limit_user(rate_limit_tags.as_ref(), None),
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
    body: Incoming,
    peer: &'a std::net::SocketAddr,
    config: &'a ApiConfig,
    full_path: &'a str,
    relative_path_components: &'a [&'a str],
    rpcenv: RestEnvironment,
    rate_limit_tags: Option<RateLimiterTagsHandle>,
    request_start: Instant,
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
            rate_limit_tags,
            request_start,
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

        let context = RequestContext {
            parts: &parts,
            config,
            full_path,
            rate_limit_tags: rate_limit_tags.as_ref(),
            request_start,
        };

        let (api_method, uri_param) = match authorize_request(
            self.router,
            &relative_path_components[1..],
            &context,
            &mut rpcenv,
        )
        .await
        {
            Ok(resolved) => resolved,
            // an unauthenticated caller gets a bare 401, it must not learn anything else
            Err(err @ RequestError::Unauthorized(_)) => return Err(err.into()),
            Err(err @ (RequestError::NotFound(_) | RequestError::Forbidden(_))) => {
                return Ok(formatter.format_error(err.into()));
            }
        };

        let auth_id = rpcenv.get_auth_id();

        let result = if api_method.protected && rpcenv.env_type == RpcEnvironmentType::PUBLIC {
            proxy_protected_request(config, api_method, parts, body, peer).await
        } else {
            handle_api_request(rpcenv, api_method, Some(formatter), parts, body, uri_param).await
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
            rate_limit_tags,
            request_start,
        }: ApiRequestData<'_>,
    ) -> Result<Response<Body>, Error> {
        if relative_path_components.is_empty() {
            http_bail!(NOT_FOUND, "invalid api path '{}'", full_path);
        }

        let context = RequestContext {
            parts: &parts,
            config,
            full_path,
            rate_limit_tags: rate_limit_tags.as_ref(),
            request_start,
        };

        let (api_method, uri_param) =
            authorize_request(self.router, relative_path_components, &context, &mut rpcenv).await?;

        let auth_id = rpcenv.get_auth_id();

        let result = if api_method.protected && rpcenv.env_type == RpcEnvironmentType::PUBLIC {
            proxy_protected_request(config, api_method, parts, body, peer).await
        } else {
            handle_api_request(rpcenv, api_method, None, parts, body, uri_param).await
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

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(name: &'static str, value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(name, value.parse().unwrap());
        headers
    }

    fn peer() -> std::net::SocketAddr {
        "192.0.2.1:1234".parse().unwrap()
    }

    fn config(real_ip_header: &str, allow_from: &[&str]) -> ApiConfig {
        let config = ApiConfig::new(".", RpcEnvironmentType::PUBLIC)
            .real_ip_header(real_ip_header.to_string());

        if allow_from.is_empty() {
            config
        } else {
            config.real_ip_allow_from(
                allow_from
                    .iter()
                    .map(|cidr| cidr.parse().unwrap())
                    .collect(),
            )
        }
    }

    struct TestUser;

    impl UserInformation for TestUser {
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

    fn dummy_api_handler(
        _param: Value,
        _info: &ApiMethod,
        _rpcenv: &mut dyn RpcEnvironment,
    ) -> Result<Value, Error> {
        Ok(Value::Null)
    }

    const HANDLER: ApiHandler = ApiHandler::Sync(&dummy_api_handler);
    const SCHEMA: proxmox_schema::ObjectSchema = proxmox_schema::ObjectSchema::new("test", &[]);

    const WORLD_METHOD: ApiMethod =
        ApiMethod::new(&HANDLER, &SCHEMA).access(None, &Permission::World);
    const ANYBODY_METHOD: ApiMethod =
        ApiMethod::new(&HANDLER, &SCHEMA).access(None, &Permission::Anybody);
    const SUPERUSER_METHOD: ApiMethod =
        ApiMethod::new(&HANDLER, &SCHEMA).access(None, &Permission::Superuser);

    static WORLD_ROUTER: Router = Router::new().get(&WORLD_METHOD);
    static ANYBODY_ROUTER: Router = Router::new().get(&ANYBODY_METHOD);
    static SUPERUSER_ROUTER: Router = Router::new().get(&SUPERUSER_METHOD);
    static EMPTY_ROUTER: Router = Router::new();

    fn request_parts() -> Parts {
        http::Request::builder()
            .method(http::Method::GET)
            .uri("/")
            .body(())
            .unwrap()
            .into_parts()
            .0
    }

    fn auth_config(authenticates: bool) -> Arc<ApiConfig> {
        Arc::new(
            ApiConfig::new(".", RpcEnvironmentType::PUBLIC).auth_handler_func(move |_, _| {
                Box::pin(async move {
                    if authenticates {
                        let user: Box<dyn UserInformation + Send + Sync> = Box::new(TestUser);
                        Ok(("tester@pam".to_string(), user))
                    } else {
                        Err(AuthError::NoData)
                    }
                })
            }),
        )
    }

    async fn authorize(
        router: &'static Router,
        config: &Arc<ApiConfig>,
        rpcenv: &mut RestEnvironment,
        request_start: Instant,
    ) -> Result<(&'static ApiMethod, HashMap<String, String>), RequestError> {
        let parts = request_parts();
        let context = RequestContext {
            parts: &parts,
            config,
            full_path: "/test",
            rate_limit_tags: None,
            request_start,
        };

        authorize_request(router, &[], &context, rpcenv).await
    }

    #[test]
    fn without_a_configured_header_the_socket_peer_is_used() {
        // the public environment does not trust any header by default
        let config = ApiConfig::new(".", RpcEnvironmentType::PUBLIC);
        let headers = headers("forwarded", r#"for="198.51.100.7:443""#);

        assert_eq!(get_proxied_peer(&headers, &config, &peer()), None);
    }

    #[test]
    fn forwarded_accepts_the_shapes_allowed_by_rfc_7239() {
        let config = config("Forwarded", &[]);

        for (header, expected) in [
            (r#"for="198.51.100.7:443""#, "198.51.100.7:443"),
            (r#"for=198.51.100.7"#, "198.51.100.7:0"),
            (r#"For="198.51.100.7""#, "198.51.100.7:0"),
            (
                r#"by=203.0.113.1;for=198.51.100.7;proto=https"#,
                "198.51.100.7:0",
            ),
            (r#"for="[2001:db8::1]:8443""#, "[2001:db8::1]:8443"),
            (r#"for="[2001:db8::1]""#, "[2001:db8::1]:0"),
        ] {
            let expected: std::net::SocketAddr = expected.parse().unwrap();
            assert_eq!(
                get_proxied_peer(&headers("forwarded", header), &config, &peer()),
                Some(expected),
                "failed for {header}"
            );
        }
    }

    #[test]
    fn a_malformed_forwarded_header_falls_back_to_the_socket_peer() {
        let config = config("Forwarded", &[]);

        for header in [
            r#"for="not-an-address""#,
            "proto=https",
            "for=",
            // the element the trusted proxy appended carries no `for` at all
            "for=198.51.100.7, proto=https",
            // an unclosed quote swallows the comma the proxy put in front of its own element,
            // so the header cannot be split and must not be guessed at
            r#"for=198.51.100.7;ext="unterminated, for=203.0.113.5"#,
            r#"for=198.51.100.7;ext="a\, for=203.0.113.5"#,
            // a dangling escape inside a quoted value leaves the same question open
            r#"for=198.51.100.7;ext="a\"#,
        ] {
            assert_eq!(
                get_proxied_peer(&headers("forwarded", header), &config, &peer()),
                None,
                "failed for {header}"
            );
        }
    }

    #[test]
    fn forwarded_only_reads_the_for_of_the_last_element() {
        let config = config("Forwarded", &[]);

        for (header, expected) in [
            // proxies append, so only the last element was added by the proxy we trust; the one
            // in front of it is whatever the client sent and must not win
            ("for=198.51.100.7, for=203.0.113.5", Some("203.0.113.5:0")),
            // a `for=` inside another parameter's value is not the `for` parameter
            (
                r#"host="my-for=1.2.3.4";for=203.0.113.5"#,
                Some("203.0.113.5:0"),
            ),
            (r#"by="for=1.2.3.4";for=203.0.113.5"#, Some("203.0.113.5:0")),
            // and neither is an extension parameter that merely ends in "for"
            ("x-for=1.2.3.4", None),
            // delimiters inside a quoted value separate nothing
            (
                r#"ext="x;for=198.51.100.7;y";for=203.0.113.5"#,
                Some("203.0.113.5:0"),
            ),
            (
                r#"ext="a,for=198.51.100.7";for=203.0.113.5"#,
                Some("203.0.113.5:0"),
            ),
            // an escaped quote does not end the quoted value
            (
                r#"ext="a\";for=198.51.100.7;b";for=203.0.113.5"#,
                Some("203.0.113.5:0"),
            ),
        ] {
            let expected = expected.map(|addr| addr.parse::<std::net::SocketAddr>().unwrap());
            assert_eq!(
                get_proxied_peer(&headers("forwarded", header), &config, &peer()),
                expected,
                "failed for {header}"
            );
        }
    }

    #[test]
    fn forwarded_uses_the_last_header_line() {
        let config = config("Forwarded", &[]);
        let mut headers = headers("forwarded", "for=198.51.100.7");
        headers.append("forwarded", "for=203.0.113.5".parse().unwrap());

        assert_eq!(
            get_proxied_peer(&headers, &config, &peer()),
            Some("203.0.113.5:0".parse().unwrap())
        );
    }

    #[test]
    fn a_custom_header_may_omit_the_port() {
        let config = config("X-Forwarded-For", &[]);

        assert_eq!(
            get_proxied_peer(
                &headers("x-forwarded-for", "198.51.100.7"),
                &config,
                &peer()
            ),
            Some("198.51.100.7:0".parse().unwrap())
        );
        assert_eq!(
            get_proxied_peer(
                &headers("x-forwarded-for", "198.51.100.7:443"),
                &config,
                &peer()
            ),
            Some("198.51.100.7:443".parse().unwrap())
        );
    }

    #[test]
    fn a_header_from_an_untrusted_peer_is_ignored() {
        let config = config("X-Forwarded-For", &["10.0.0.0/8"]);
        let headers = headers("x-forwarded-for", "198.51.100.7");

        // 192.0.2.1 is not part of the trusted network, so it must not get to pick its own IP
        assert_eq!(get_proxied_peer(&headers, &config, &peer()), None);

        let trusted = "10.1.2.3:1234".parse().unwrap();
        assert_eq!(
            get_proxied_peer(&headers, &config, &trusted),
            Some("198.51.100.7:0".parse().unwrap())
        );
    }

    #[test]
    fn the_delays_are_measured_from_the_start_of_the_request() {
        let start = Instant::now() - std::time::Duration::from_secs(60);

        // computing them from the moment of failure instead would leak how long the check ran
        assert_eq!(
            delay_unauth_time(start) - start,
            std::time::Duration::from_millis(3000)
        );
        assert_eq!(
            access_forbidden_time(start) - start,
            std::time::Duration::from_millis(500)
        );
    }

    #[test]
    fn compression_is_only_picked_up_from_accept_encoding() {
        let deflate = headers("accept-encoding", "deflate");
        assert_eq!(
            extract_compression_method(&deflate),
            Some(CompressionMethod::Deflate)
        );

        let weighted = headers("accept-encoding", "br, gzip;q=0.8, deflate;q=0.5");
        assert_eq!(
            extract_compression_method(&weighted),
            Some(CompressionMethod::Deflate)
        );

        assert_eq!(
            extract_compression_method(&headers("accept-encoding", "gzip")),
            None
        );
        assert_eq!(extract_compression_method(&HeaderMap::new()), None);
    }

    #[test]
    fn compression_weights_are_not_split_from_their_coding() {
        for value in ["deflate ;q=0", "deflate; q=0", "gzip, deflate ; q = 0"] {
            assert_eq!(
                extract_compression_method(&headers("accept-encoding", value)),
                None,
                "accepted {value}"
            );
        }
    }

    #[test]
    fn content_types_are_derived_from_the_file_extension() {
        for (file, content_type, no_compression) in [
            ("index.html", "text/html", false),
            ("app.js", "application/javascript", false),
            ("logo.png", "image/png", true),
            ("font.woff2", "application/font-woff2", true),
            ("archive", "application/octet-stream", false),
            ("unknown.xyz", "application/octet-stream", false),
        ] {
            assert_eq!(
                extension_to_content_type(Path::new(file)),
                (content_type, no_compression),
                "failed for {file}"
            );
        }
    }

    #[test]
    fn the_user_agent_is_truncated() {
        let long = "x".repeat(200);
        let agent = get_user_agent(&headers("user-agent", &long)).unwrap();

        assert_eq!(agent.len(), 128);
    }

    #[tokio::test]
    async fn a_world_endpoint_needs_no_credentials() {
        let config = Arc::new(
            ApiConfig::new(".", RpcEnvironmentType::PUBLIC).auth_handler_func(|_, _| {
                panic!("world-accessible methods must not invoke authentication")
            }),
        );
        let mut rpcenv = RestEnvironment::new(RpcEnvironmentType::PUBLIC, Arc::clone(&config));

        let result = authorize(&WORLD_ROUTER, &config, &mut rpcenv, Instant::now()).await;

        assert!(result.is_ok());
        assert_eq!(rpcenv.get_auth_id(), None);
    }

    #[tokio::test]
    async fn an_authenticated_caller_reaches_the_method() {
        let config = auth_config(true);
        let mut rpcenv = RestEnvironment::new(RpcEnvironmentType::PUBLIC, Arc::clone(&config));

        let result = authorize(&ANYBODY_ROUTER, &config, &mut rpcenv, Instant::now()).await;

        assert!(result.is_ok());
        assert_eq!(rpcenv.get_auth_id().as_deref(), Some("tester@pam"));
    }

    #[tokio::test]
    async fn routing_parameters_are_used_for_authorization() {
        const USERID: proxmox_schema::Schema =
            proxmox_schema::StringSchema::new("The user.").schema();
        const USERID_SCHEMA: proxmox_schema::ObjectSchema =
            proxmox_schema::ObjectSchema::new("test", &[("userid", false, &USERID)]);
        const METHOD: ApiMethod =
            ApiMethod::new(&HANDLER, &USERID_SCHEMA).access(None, &Permission::UserParam("userid"));
        static ROUTER: Router = Router::new().match_all("userid", &Router::new().get(&METHOD));
        let config = auth_config(true);
        let mut rpcenv = RestEnvironment::new(RpcEnvironmentType::PUBLIC, Arc::clone(&config));
        let parts = request_parts();
        let context = RequestContext {
            parts: &parts,
            config: &config,
            full_path: "/tester@pam",
            rate_limit_tags: None,
            request_start: Instant::now(),
        };

        let result = authorize_request(&ROUTER, &["tester@pam"], &context, &mut rpcenv).await;
        let Ok((_, params)) = result else {
            panic!("the path's user must be allowed to access their own method");
        };
        assert_eq!(params.get("userid").map(String::as_str), Some("tester@pam"));
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_authentication_is_delayed() {
        use std::time::Duration;

        let config = auth_config(false);
        let mut rpcenv = RestEnvironment::new(RpcEnvironmentType::PUBLIC, Arc::clone(&config));
        let start = Instant::now();
        tokio::time::advance(Duration::from_secs(1)).await;

        let pending = authorize(&ANYBODY_ROUTER, &config, &mut rpcenv, start);
        futures::pin_mut!(pending);
        assert!(futures::poll!(pending.as_mut()).is_pending());
        tokio::time::advance(Duration::from_millis(1999)).await;
        assert!(futures::poll!(pending.as_mut()).is_pending());
        // Allow one millisecond for Tokio's timer granularity, but not a fresh three-second delay.
        tokio::time::advance(Duration::from_millis(2)).await;
        assert!(matches!(
            futures::poll!(pending.as_mut()),
            Poll::Ready(Err(RequestError::Unauthorized(_)))
        ));
    }

    #[tokio::test]
    async fn an_unknown_path_is_authenticated_before_it_is_reported_missing() {
        let config = auth_config(true);
        let mut rpcenv = RestEnvironment::new(RpcEnvironmentType::PUBLIC, Arc::clone(&config));

        let result = authorize(&EMPTY_ROUTER, &config, &mut rpcenv, Instant::now()).await;

        assert!(matches!(result, Err(RequestError::NotFound(_))));
        // an unknown path must not be a way around authentication
        assert_eq!(rpcenv.get_auth_id().as_deref(), Some("tester@pam"));
    }

    #[tokio::test(start_paused = true)]
    async fn a_caller_without_the_privilege_is_forbidden() {
        use std::time::Duration;

        let config = auth_config(true);
        let mut rpcenv = RestEnvironment::new(RpcEnvironmentType::PUBLIC, Arc::clone(&config));
        let start = Instant::now();
        tokio::time::advance(Duration::from_millis(200)).await;

        let pending = authorize(&SUPERUSER_ROUTER, &config, &mut rpcenv, start);
        futures::pin_mut!(pending);
        assert!(futures::poll!(pending.as_mut()).is_pending());
        tokio::time::advance(Duration::from_millis(299)).await;
        assert!(futures::poll!(pending.as_mut()).is_pending());
        tokio::time::advance(Duration::from_millis(2)).await;
        assert!(matches!(
            futures::poll!(pending.as_mut()),
            Poll::Ready(Err(RequestError::Forbidden(_)))
        ));
    }
}
