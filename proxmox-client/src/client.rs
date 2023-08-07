use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;

use http::request::Request;
use http::response::Response;
use http::uri::PathAndQuery;
use http::{StatusCode, Uri};
use serde_json::Value;

use proxmox_login::{Login, SecondFactorChallenge, TicketResult};

use crate::auth::AuthenticationKind;
use crate::{Error, Token};

/// HTTP client backend trait.
///
/// An async [`Client`] requires some kind of async HTTP client implementation.
pub trait HttpClient: Send + Sync {
    type ResponseFuture: Future<Output = Result<Response<Vec<u8>>, Error>>;

    fn request(&self, request: Request<Vec<u8>>) -> Self::ResponseFuture;
}

/// Proxmox VE high level API client.
pub struct Client<C> {
    api_url: Uri,
    auth: Mutex<Option<Arc<AuthenticationKind>>>,
    client: C,
    pve_compat: bool,
}

impl<C> Client<C> {
    /// Get the underlying client object.
    pub fn inner(&self) -> &C {
        &self.client
    }

    /// Get a mutable reference to the underlying client object.
    pub fn inner_mut(&mut self) -> &mut C {
        &mut self.client
    }

    /// Get a reference to the current authentication information.
    pub fn authentication(&self) -> Option<Arc<AuthenticationKind>> {
        self.auth.lock().unwrap().clone()
    }

    pub fn use_api_token(&self, token: Token) {
        *self.auth.lock().unwrap() = Some(Arc::new(token.into()));
    }
}

fn to_request(request: proxmox_login::Request) -> Result<http::Request<Vec<u8>>, Error> {
    http::Request::builder()
        .method(http::Method::POST)
        .uri(request.url)
        .header(http::header::CONTENT_TYPE, request.content_type)
        .header(
            http::header::CONTENT_LENGTH,
            request.content_length.to_string(),
        )
        .body(request.body.into_bytes())
        .map_err(|err| Error::internal("error building login http request", err))
}

impl<C> Client<C> {
    /// Enable Proxmox VE login API compatibility. This is required to support TFA authentication
    /// on Proxmox VE APIs which require the `new-format` option.
    pub fn set_pve_compatibility(&mut self, compatibility: bool) {
        self.pve_compat = compatibility;
    }
}

impl<C> Client<C>
where
    C: HttpClient,
{
    /// Instantiate a client for an API with a given HTTP client instance.
    pub fn with_client(api_url: Uri, client: C) -> Self {
        Self {
            api_url,
            auth: Mutex::new(None),
            client,
            pve_compat: false,
        }
    }

    /// Assert that we are authenticated and return the `AuthenticationKind`.
    /// Otherwise returns `Error::Unauthenticated`.
    pub fn login_auth(&self) -> Result<Arc<AuthenticationKind>, Error> {
        self.auth
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| Error::Unauthorized)
    }

    /// If currently logged in, this will fill in the auth cookie and CSRFPreventionToken header
    /// and return `Ok(request)`, otherwise it'll return `Err(request)` with the request
    /// unmodified.
    pub fn try_set_auth_headers(
        &self,
        request: http::request::Builder,
    ) -> Result<http::request::Builder, http::request::Builder> {
        let auth = self.auth.lock().unwrap().clone();
        match auth {
            Some(auth) => Ok(auth.set_auth_headers(request)),
            None => Err(request),
        }
    }

    /// Convenience method to login and set the authentication headers for a request.
    pub async fn set_auth_headers(
        &self,
        request: http::request::Builder,
    ) -> Result<http::request::Builder, Error> {
        Ok(self.login_auth()?.set_auth_headers(request))
    }

    /// Attempt to login.
    ///
    /// This will propagate the PVE compatibility state and then perform the `Login` request via
    /// the inner http client.
    ///
    /// If the authentication is complete, `None` is returned and the authentication state updated.
    /// If a 2nd factor is required, `Some` is returned.
    pub async fn login(&self, login: Login) -> Result<Option<SecondFactorChallenge>, Error> {
        let login = login.pve_compatibility(self.pve_compat);

        let api_response = self.client.request(to_request(login.request())?).await?;

        if !api_response.status().is_success() {
            // FIXME: does `http` somehow expose the status string?
            return Err(Error::api(api_response.status(), "authentication failed"));
        }

        Ok(match login.response(api_response.body())? {
            TicketResult::TfaRequired(challenge) => Some(challenge),
            TicketResult::Full(auth) => {
                *self.auth.lock().unwrap() = Some(Arc::new(auth.into()));
                None
            }
        })
    }

    /// Attempt to finish a 2nd factor login.
    ///
    /// This will propagate the PVE compatibility state and then perform the `Login` request via
    /// the inner http client.
    pub async fn login_tfa(
        &self,
        challenge: SecondFactorChallenge,
        challenge_response: proxmox_login::Request,
    ) -> Result<(), Error> {
        let api_response = self.client.request(to_request(challenge_response)?).await?;

        if !api_response.status().is_success() {
            // FIXME: does `http` somehow expose the status string?
            return Err(Error::api(api_response.status(), "authentication failed"));
        }

        let auth = challenge.response(api_response.body())?;
        *self.auth.lock().unwrap() = Some(Arc::new(auth.into()));
        Ok(())
    }

    /// Get the currently used API url.
    pub fn api_url(&self) -> &Uri {
        &self.api_url
    }

    /// Build a URI relative to the current API endpoint.
    fn build_uri(&self, path: &str) -> Result<Uri, Error> {
        let parts = self.api_url.clone().into_parts();
        let mut builder = http::uri::Builder::new();
        if let Some(scheme) = parts.scheme {
            builder = builder.scheme(scheme);
        }
        if let Some(authority) = parts.authority {
            builder = builder.authority(authority)
        }
        builder
            .path_and_query(
                path.parse::<PathAndQuery>()
                    .map_err(|err| Error::internal("failed to parse uri", err))?,
            )
            .build()
            .map_err(|err| Error::internal("failed to build Uri", err))
    }

    /// Execute a `GET` request, possibly trying multiple cluster nodes.
    pub async fn get<'a, R>(&'a self, uri: &str) -> Result<ApiResponse<R>, Error>
    where
        R: serde::de::DeserializeOwned,
    {
        let request = self
            .set_auth_headers(Request::get(self.build_uri(uri)?))
            .await?
            .body(Vec::new())
            .map_err(|err| Error::internal("failed to build request", err))?;

        Self::handle_response(self.client.request(request).await?)
    }

    /// Execute a `GET` request with the given body, possibly trying multiple cluster nodes.
    pub async fn get_with_body<'a, B, R>(
        &'a self,
        uri: &str,
        body: &'a B,
    ) -> Result<ApiResponse<R>, Error>
    where
        B: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let auth = self.login_auth()?;
        self.json_request(&auth, http::Method::GET, uri, body).await
    }

    /// Execute a `PUT` request with the given body, possibly trying multiple cluster nodes.
    pub async fn put<'a, B, R>(&'a self, uri: &str, body: &'a B) -> Result<ApiResponse<R>, Error>
    where
        B: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let auth = self.login_auth()?;
        self.json_request(&auth, http::Method::PUT, uri, body).await
    }

    /// Execute a `POST` request with the given body, possibly trying multiple cluster nodes.
    pub async fn post<'a, B, R>(&'a self, uri: &str, body: &'a B) -> Result<ApiResponse<R>, Error>
    where
        B: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let auth = self.login_auth()?;
        self.json_request(&auth, http::Method::POST, uri, body)
            .await
    }

    /// Execute a `DELETE` request, possibly trying multiple cluster nodes.
    pub async fn delete<'a, R>(&'a self, uri: &str) -> Result<ApiResponse<R>, Error>
    where
        R: serde::de::DeserializeOwned,
    {
        let request = self
            .set_auth_headers(Request::delete(self.build_uri(uri)?))
            .await?
            .body(Vec::new())
            .map_err(|err| Error::internal("failed to build request", err))?;

        Self::handle_response(self.client.request(request).await?)
    }

    /// Execute a `DELETE` request with the given body, possibly trying multiple cluster nodes.
    pub async fn delete_with_body<'a, B, R>(
        &'a self,
        uri: &str,
        body: &'a B,
    ) -> Result<ApiResponse<R>, Error>
    where
        B: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let auth = self.login_auth()?;
        self.json_request(&auth, http::Method::DELETE, uri, body)
            .await
    }

    /// Helper method for a JSON request with a JSON body `B`, yielding a JSON result type `R`.
    pub(crate) async fn json_request<'a, B, R>(
        &'a self,
        auth: &'a AuthenticationKind,
        method: http::Method,
        uri: &str,
        body: &'a B,
    ) -> Result<ApiResponse<R>, Error>
    where
        B: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let body = serde_json::to_vec(&body)
            .map_err(|err| Error::internal("failed to serialize request body", err))?;
        let content_length = body.len();
        self.json_request_bytes(auth, method, uri, body, content_length)
            .await
    }

    /// Helper method for a request with a byte body, yieldinig a JSON result of type `R`.
    async fn json_request_bytes<'a, R>(
        &'a self,
        auth: &AuthenticationKind,
        method: http::Method,
        uri: &str,
        body: Vec<u8>,
        content_length: usize,
    ) -> Result<ApiResponse<R>, Error>
    where
        R: serde::de::DeserializeOwned,
    {
        let response = self
            .run_json_request_with_body(auth, method, uri, body, content_length)
            .await?;
        Self::handle_response(response)
    }

    async fn run_json_request_with_body<'a>(
        &'a self,
        auth: &'a AuthenticationKind,
        method: http::Method,
        uri: &str,
        body: Vec<u8>,
        content_length: usize,
    ) -> Result<Response<Vec<u8>>, Error> {
        let request = Request::builder()
            .method(method.clone())
            .uri(self.build_uri(uri)?)
            .header(http::header::CONTENT_TYPE, "application/json")
            .header(http::header::CONTENT_LENGTH, content_length.to_string());

        let request = auth
            .set_auth_headers(request)
            .body(body.clone())
            .map_err(|err| Error::internal("failed to build request", err))?;

        Ok(self.client.request(request).await?)
    }

    /// Check the status code, deserialize the json/extjs `RawApiResponse` and check for error
    /// messages inside.
    /// On success, deserialize the expected result type.
    fn handle_response<R>(response: Response<Vec<u8>>) -> Result<ApiResponse<R>, Error>
    where
        R: serde::de::DeserializeOwned,
    {
        if response.status() == StatusCode::UNAUTHORIZED {
            return Err(Error::Unauthorized);
        }

        if !response.status().is_success() {
            // FIXME: Decode json errors...
            //match serde_json::from_slice(&body)
            //    Ok(value) =>
            //        if value["error"]
            let (response, body) = response.into_parts();
            let body =
                String::from_utf8(body).map_err(|_| Error::Other("API returned non-utf8 data"))?;
            return Err(Error::api(response.status, body));
        }

        let data: RawApiResponse<R> = serde_json::from_slice(&response.into_body())
            .map_err(|err| Error::internal("failed to deserialize api response", err))?;

        data.check()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NoData;

impl std::error::Error for NoData {}
impl fmt::Display for NoData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("api returned no data")
    }
}

pub struct ApiResponse<T> {
    pub data: Option<T>,
    pub attribs: HashMap<String, Value>,
}

impl<T> ApiResponse<T> {
    pub fn into_data_or_err(mut self) -> Result<T, NoData> {
        self.data.take().ok_or(NoData)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct UnexpectedData;

impl std::error::Error for UnexpectedData {}
impl fmt::Display for UnexpectedData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("api returned unexpected data")
    }
}

impl ApiResponse<()> {
    pub fn nodata(self) -> Result<(), UnexpectedData> {
        if self.data.is_some() {
            Err(UnexpectedData)
        } else {
            Ok(())
        }
    }
}

#[derive(serde::Deserialize)]
struct RawApiResponse<T> {
    #[serde(default, deserialize_with = "proxmox_login::parse::deserialize_u16")]
    pub status: Option<u16>,
    pub message: Option<String>,
    #[serde(default, deserialize_with = "proxmox_login::parse::deserialize_bool")]
    pub success: Option<bool>,
    pub data: Option<T>,

    #[serde(default)]
    pub errors: HashMap<String, String>,

    #[serde(default, flatten)]
    pub attribs: HashMap<String, Value>,
}

impl<T> RawApiResponse<T> {
    pub fn check(mut self) -> Result<ApiResponse<T>, Error> {
        if !self.success.unwrap_or(false) {
            let status = http::StatusCode::from_u16(self.status.unwrap_or(400))
                .unwrap_or(http::StatusCode::BAD_REQUEST);
            let mut message = self
                .message
                .take()
                .unwrap_or_else(|| "no message provided".to_string());
            for (param, error) in self.errors {
                use std::fmt::Write;
                let _ = write!(message, "\n{param}: {error}");
            }

            return Err(Error::api(status, message));
        }

        Ok(ApiResponse {
            data: self.data,
            attribs: self.attribs,
        })
    }
}

#[cfg(feature = "hyper-client")]
pub type HyperClient = Client<Arc<proxmox_http::client::Client>>;

#[cfg(feature = "hyper-client")]
impl<C> Client<C> {
    /// Create a new client instance which will connect to the provided endpoint.
    pub fn new(api_url: Uri) -> HyperClient {
        Client::with_client(api_url, Arc::new(proxmox_http::client::Client::new()))
    }
}

#[cfg(feature = "hyper-client")]
mod hyper_client_extras {
    use std::future::Future;
    use std::sync::Arc;

    use http::request::Request;
    use http::response::Response;
    use http::Uri;
    use openssl::hash::MessageDigest;
    use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
    use openssl::x509::{self, X509};

    use proxmox_http::client::Client as ProxmoxClient;

    use super::{Client, HyperClient};
    use crate::Error;

    #[derive(Default)]
    pub enum TlsOptions {
        /// Default TLS verification.
        #[default]
        Verify,

        /// Insecure: ignore invalid certificates.
        Insecure,

        /// Expect a specific certificate fingerprint.
        Fingerprint(Vec<u8>),

        /// Verify with a specific PEM formatted CA.
        CaCert(X509),

        /// Use a callback for certificate verification.
        Callback(Box<dyn Fn(bool, &mut x509::X509StoreContextRef) -> bool + Send + Sync + 'static>),
    }

    fn fp_string(fp: &[u8]) -> String {
        use std::fmt::Write as _;

        let mut out = String::new();
        for b in fp {
            if !out.is_empty() {
                out.push(':');
            }
            let _ = write!(out, "{b:02x}");
        }
        out
    }

    fn verify_fingerprint(chain: &x509::X509StoreContextRef, expected_fingerprint: &[u8]) -> bool {
        let Some(cert) = chain.current_cert() else {
            log::error!("no certificate in chain?");
            return false;
        };

        let fp = match cert.digest(MessageDigest::sha256()) {
            Err(err) => {
                log::error!("error calculating certificate fingerprint: {err}");
                return false;
            }
            Ok(fp) => fp,
        };

        if expected_fingerprint != fp.as_ref() {
            log::error!("bad fingerprint: {}", fp_string(&fp));
            log::error!("expected fingerprint: {}", fp_string(&expected_fingerprint));
            return false;
        }

        true
    }

    impl<C> Client<C> {
        /// Create a new client instance which will connect to the provided endpoint.
        pub fn with_options(
            api_url: Uri,
            tls_options: TlsOptions,
            http_options: proxmox_http::HttpOptions,
        ) -> Result<HyperClient, Error> {
            let mut connector = SslConnector::builder(SslMethod::tls_client())
                .map_err(|err| Error::internal("failed to create ssl connector builder", err))?;

            match tls_options {
                TlsOptions::Verify => (),
                TlsOptions::Insecure => connector.set_verify(SslVerifyMode::NONE),
                TlsOptions::Fingerprint(expected_fingerprint) => {
                    connector.set_verify_callback(SslVerifyMode::PEER, move |valid, chain| {
                        if valid {
                            return true;
                        }
                        verify_fingerprint(chain, &expected_fingerprint)
                    });
                }
                TlsOptions::Callback(cb) => {
                    connector.set_verify_callback(SslVerifyMode::PEER, move |valid, chain| {
                        cb(valid, chain)
                    });
                }
                TlsOptions::CaCert(ca) => {
                    let mut store =
                        openssl::x509::store::X509StoreBuilder::new().map_err(|err| {
                            Error::internal("failed to create certificate store builder", err)
                        })?;
                    store
                        .add_cert(ca)
                        .map_err(|err| Error::internal("failed to build certificate store", err))?;
                    connector.set_cert_store(store.build());
                }
            }

            let client = ProxmoxClient::with_ssl_connector(connector.build(), http_options);

            Ok(Client::with_client(api_url, Arc::new(client)))
        }
    }

    impl super::HttpClient for Arc<proxmox_http::client::Client> {
        #[allow(clippy::type_complexity)]
        type ResponseFuture =
            std::pin::Pin<Box<dyn Future<Output = Result<Response<Vec<u8>>, Error>> + Send>>;

        fn request(&self, request: Request<Vec<u8>>) -> Self::ResponseFuture {
            let (parts, body) = request.into_parts();
            let request = Request::<hyper::Body>::from_parts(parts, body.into());
            let this = Arc::clone(self);
            Box::pin(async move {
                use hyper::body::HttpBody;

                // FIXME: proxmox_http's client needs a way to return http status codes and such...
                let (response, mut body) = (*this)
                    .request(request)
                    .await
                    .map_err(Error::Anyhow)?
                    .into_parts();

                let mut data = Vec::<u8>::new();
                while let Some(more) = body.data().await {
                    let more = more.map_err(|err| Error::internal("error reading body", err))?;
                    data.extend(&more[..]);
                }

                Ok::<_, Error>(Response::from_parts(response, data))
            })
        }
    }
}

#[cfg(feature = "hyper-client")]
pub use hyper_client_extras::TlsOptions;
