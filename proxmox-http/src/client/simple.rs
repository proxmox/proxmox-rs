use anyhow::{bail, format_err, Error};
use std::collections::HashMap;

use std::io::Read;
#[cfg(all(feature = "client-trait", feature = "proxmox-async"))]
use std::str::FromStr;

use futures::*;
#[cfg(all(feature = "client-trait", feature = "proxmox-async"))]
use http::header::HeaderName;
use http::{HeaderValue, Request, Response};
use hyper::client::Client as HyperClient;
use hyper::client::HttpConnector;
use hyper::Body;
use openssl::ssl::{SslConnector, SslMethod};

use crate::client::HttpsConnector;
use crate::HttpOptions;

/// Asynchronous HTTP client implementation
pub struct Client {
    client: HyperClient<HttpsConnector, Body>,
    options: HttpOptions,
}

impl Client {
    pub const DEFAULT_USER_AGENT_STRING: &'static str = "proxmox-simple-http-client/0.1";

    pub fn new() -> Self {
        Self::with_options(HttpOptions::default())
    }

    pub fn with_options(options: HttpOptions) -> Self {
        let ssl_connector = SslConnector::builder(SslMethod::tls()).unwrap().build();
        Self::with_ssl_connector(ssl_connector, options)
    }

    pub fn with_ssl_connector(ssl_connector: SslConnector, options: HttpOptions) -> Self {
        let connector = HttpConnector::new();
        let mut https = HttpsConnector::with_connector(
            connector,
            ssl_connector,
            options.tcp_keepalive.unwrap_or(7200),
        );
        if let Some(ref proxy_config) = options.proxy_config {
            https.set_proxy(proxy_config.clone());
        }
        let client = HyperClient::builder().build(https);
        Self { client, options }
    }

    pub fn set_user_agent(&mut self, user_agent: &str) -> Result<(), Error> {
        self.options.user_agent = Some(user_agent.to_owned());
        Ok(())
    }

    fn add_proxy_headers(&self, request: &mut Request<Body>) -> Result<(), Error> {
        if request.uri().scheme() != Some(&http::uri::Scheme::HTTPS) {
            if let Some(ref authorization) = self.options.get_proxy_authorization() {
                request.headers_mut().insert(
                    http::header::PROXY_AUTHORIZATION,
                    HeaderValue::from_str(authorization)?,
                );
            }
        }
        Ok(())
    }

    pub async fn request(&self, mut request: Request<Body>) -> Result<Response<Body>, Error> {
        let user_agent = if let Some(user_agent) = &self.options.user_agent {
            HeaderValue::from_str(user_agent)?
        } else {
            HeaderValue::from_str(Self::DEFAULT_USER_AGENT_STRING)?
        };

        request
            .headers_mut()
            .insert(hyper::header::USER_AGENT, user_agent);

        self.add_proxy_headers(&mut request)?;

        self.client.request(request).map_err(Error::from).await
    }

    pub async fn post<R>(
        &self,
        uri: &str,
        body: Option<R>,
        content_type: Option<&str>,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<Response<Body>, Error>
    where
        R: Read,
    {
        let body = if let Some(mut body) = body {
            let mut body_vec = Vec::new();
            body.read_to_end(&mut body_vec)?;
            Body::from(body_vec)
        } else {
            Body::empty()
        };
        let content_type = content_type.unwrap_or("application/json");

        let mut request = Request::builder()
            .method("POST")
            .uri(uri)
            .header(hyper::header::CONTENT_TYPE, content_type);

        if let Some(extra_headers) = extra_headers {
            for (header, value) in extra_headers {
                request = request.header(header, value);
            }
        }

        let request = request.body(body)?;

        self.request(request).await
    }

    pub async fn get_string(
        &self,
        uri: &str,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<String, Error> {
        let mut request = Request::builder().method("GET").uri(uri);

        if let Some(hs) = extra_headers {
            for (h, v) in hs.iter() {
                request = request.header(h, v);
            }
        }

        let request = request.body(Body::empty())?;

        let res = self.request(request).await?;

        let status = res.status();
        if !status.is_success() {
            bail!("Got bad status '{}' from server", status)
        }

        Self::response_body_string(res).await
    }

    pub async fn response_body_string(res: Response<Body>) -> Result<String, Error> {
        Self::convert_body_to_string(Ok(res))
            .await
            .map(|res| res.into_body())
    }

    async fn convert_body_to_string(
        response: Result<Response<Body>, Error>,
    ) -> Result<Response<String>, Error> {
        match response {
            Ok(res) => {
                let (parts, body) = res.into_parts();

                let buf = hyper::body::to_bytes(body).await?;
                let new_body = String::from_utf8(buf.to_vec())
                    .map_err(|err| format_err!("Error converting HTTP result data: {}", err))?;

                Ok(Response::from_parts(parts, new_body))
            }
            Err(err) => Err(err),
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(feature = "client-trait", feature = "proxmox-async"))]
impl crate::HttpClient<Body> for Client {
    fn get(
        &self,
        uri: &str,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<Response<Body>, Error> {
        let mut req = Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())?;

        if let Some(extra_headers) = extra_headers {
            let headers = req.headers_mut();
            for (header, value) in extra_headers {
                headers.insert(HeaderName::from_str(header)?, HeaderValue::from_str(value)?);
            }
        }

        proxmox_async::runtime::block_on(self.request(req))
    }

    fn post<R>(
        &self,
        uri: &str,
        body: Option<R>,
        content_type: Option<&str>,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<Response<Body>, Error>
    where
        R: Read,
    {
        proxmox_async::runtime::block_on(self.post(uri, body, content_type, extra_headers))
    }

    fn request(&self, request: Request<Body>) -> Result<Response<Body>, Error> {
        proxmox_async::runtime::block_on(async move { self.request(request).await })
    }
}

#[cfg(all(feature = "client-trait", feature = "proxmox-async"))]
impl crate::HttpClient<String> for Client {
    fn get(
        &self,
        uri: &str,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<Response<String>, Error> {
        let mut req = Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())?;

        if let Some(extra_headers) = extra_headers {
            let headers = req.headers_mut();
            for (header, value) in extra_headers {
                headers.insert(HeaderName::from_str(header)?, HeaderValue::from_str(value)?);
            }
        }

        proxmox_async::runtime::block_on(async move {
            Self::convert_body_to_string(self.request(req).await).await
        })
    }

    fn post<R>(
        &self,
        uri: &str,
        body: Option<R>,
        content_type: Option<&str>,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<Response<String>, Error>
    where
        R: Read,
    {
        proxmox_async::runtime::block_on(async move {
            Self::convert_body_to_string(self.post(uri, body, content_type, extra_headers).await)
                .await
        })
    }

    fn request(&self, request: Request<String>) -> Result<Response<String>, Error> {
        proxmox_async::runtime::block_on(async move {
            let (parts, body) = request.into_parts();
            let body = Body::from(body);
            let request = Request::from_parts(parts, body);
            Self::convert_body_to_string(self.request(request).await).await
        })
    }
}
