use anyhow::{bail, format_err, Error};
use std::collections::HashMap;

use std::fmt::Display;

#[cfg(all(feature = "client-trait", feature = "proxmox-async"))]
use http::header::HeaderName;
#[cfg(all(feature = "client-trait", feature = "proxmox-async"))]
use std::str::FromStr;

use futures::*;

use http::{HeaderValue, Request, Response};
use http_body_util::{BodyDataStream, BodyExt};
use hyper_util::client::legacy::connect::dns::GaiResolver;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client as HyperClient;
use hyper_util::rt::TokioExecutor;
use openssl::ssl::{SslConnector, SslMethod};

use crate::client::HttpsConnector;
use crate::Body;
use crate::HttpOptions;

/// Asynchronous HTTP client implementation
pub struct Client {
    client: HyperClient<HttpsConnector<GaiResolver>, Body>,
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

        let client =
            HyperClient::builder(TokioExecutor::new()).build::<HttpsConnector, Body>(https);
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
            .insert(http::header::USER_AGENT, user_agent);

        self.add_proxy_headers(&mut request)?;

        let encoded_response = self.client.request(request).map_err(Error::from).await?;
        decode_response(encoded_response).await
    }

    pub async fn post(
        &self,
        uri: &str,
        body: Option<Body>,
        content_type: Option<&str>,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<Response<Body>, Error> {
        let content_type = content_type.unwrap_or("application/json");

        let mut request = Request::builder()
            .method("POST")
            .uri(uri)
            .header(http::header::CONTENT_TYPE, content_type);

        if let Some(extra_headers) = extra_headers {
            for (header, value) in extra_headers {
                request = request.header(header, value);
            }
        }

        let body = body.unwrap_or(Body::empty());
        self.request(request.body(body)?).await
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

                let buf = body.collect().await?.to_bytes();
                let new_body = String::from_utf8(buf.to_vec())
                    .map_err(|err| format_err!("Error converting HTTP result data: {}", err))?;

                Ok(Response::from_parts(parts, new_body))
            }
            Err(err) => Err(err),
        }
    }

    pub async fn response_body_bytes(res: Response<Body>) -> Result<Body, Error> {
        Self::convert_body_to_bytes(Ok(res))
            .await
            .map(|res| res.into_body())
    }

    async fn convert_body_to_bytes(
        response: Result<Response<Body>, Error>,
    ) -> Result<Response<Body>, Error> {
        let (parts, body) = response?.into_parts();
        let buf = body.collect().await?.to_bytes();
        Ok(Response::from_parts(parts, buf.into()))
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(feature = "client-trait", feature = "proxmox-async"))]
impl crate::HttpClient<Body, Body> for Client {
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

        proxmox_async::runtime::block_on(async move {
            Self::convert_body_to_bytes(self.request(req).await).await
        })
    }

    fn post(
        &self,
        uri: &str,
        body: Option<Body>,
        content_type: Option<&str>,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<Response<Body>, Error> {
        proxmox_async::runtime::block_on(async move {
            Self::convert_body_to_bytes(self.post(uri, body, content_type, extra_headers).await)
                .await
        })
    }

    fn request(&self, request: Request<Body>) -> Result<Response<Body>, Error> {
        proxmox_async::runtime::block_on(async move {
            Self::convert_body_to_bytes(self.request(request).await).await
        })
    }
}

#[cfg(all(feature = "client-trait", feature = "proxmox-async"))]
impl crate::HttpClient<String, String> for Client {
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

    fn post(
        &self,
        uri: &str,
        body: Option<String>,
        content_type: Option<&str>,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<Response<String>, Error> {
        proxmox_async::runtime::block_on(async move {
            let body = body.map(|s| s.into());
            Self::convert_body_to_string(self.post(uri, body, content_type, extra_headers).await)
                .await
        })
    }

    fn request(&self, request: Request<String>) -> Result<Response<String>, Error> {
        proxmox_async::runtime::block_on(async move {
            let (parts, body) = request.into_parts();
            let body = body.into();
            let request = Request::from_parts(parts, body);
            Self::convert_body_to_string(self.request(request).await).await
        })
    }
}

/// Wraps the `Response` contents in a DeflateDecoder stream if the `Content-Encoding`
/// header of the response is `deflate`, otherwise returns the original
/// response.
async fn decode_response<B>(mut res: Response<B>) -> Result<Response<Body>, Error>
where
    B: http_body::Body<Data = bytes::Bytes> + Send + Unpin + 'static,
    <B as http_body::Body>::Error: Into<Error> + Display,
{
    let Some(content_encoding) = res.headers_mut().remove(&http::header::CONTENT_ENCODING) else {
        let (parts, body) = res.into_parts();
        let stream = BodyDataStream::new(body);
        let body = Body::wrap_stream(stream);
        return Ok(Response::from_parts(parts, body));
    };

    let encodings = content_encoding.to_str()?;
    if encodings == "deflate" {
        let (parts, body) = res.into_parts();

        let stream = BodyDataStream::new(body);
        let decoder = proxmox_compression::DeflateDecoder::builder(stream)
            .zlib(true)
            .build();
        let decoded_body = Body::wrap_stream(decoder);
        Ok(Response::from_parts(parts, decoded_body))
    } else {
        bail!("Unknown encoding format: {encodings}");
    }
}

#[cfg(test)]
mod test {
    use bytes::Bytes;

    use super::*;

    use std::io::Write;

    const BODY: &str = r#"Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
eiusmod tempor incididunt ut labore et dolore magnam aliquam quaerat voluptatem. Ut
enim aeque doleamus animo, cum corpore dolemus, fieri tamen permagna accessio potest,
si aliquod aeternum et infinitum impendere."#;

    #[tokio::test]
    async fn test_parse_response_deflate() {
        let encoded = encode_deflate(BODY.as_bytes()).unwrap();
        let encoded_body = Body::from(Bytes::from(encoded));
        let encoded_response = Response::builder()
            .header(http::header::CONTENT_ENCODING, "deflate")
            .body::<Body>(encoded_body)
            .unwrap();

        let decoded_response = decode_response(encoded_response).await.unwrap();

        assert_eq!(
            Client::response_body_string(decoded_response)
                .await
                .unwrap(),
            BODY
        );
    }

    fn encode_deflate(bytes: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        use flate2::write::ZlibEncoder;
        use flate2::Compression;

        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(bytes).unwrap();

        e.finish()
    }
}
