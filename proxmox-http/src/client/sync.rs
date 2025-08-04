use std::collections::HashMap;
use std::io::Read;
use std::time::Duration;

use anyhow::Error;
use http::Response;

use crate::HttpClient;
use crate::HttpOptions;

#[derive(Default)]
/// Blocking HTTP client for usage with [`HttpClient`].
pub struct Client {
    options: HttpOptions,
    timeout: Option<Duration>,
}

impl Client {
    pub fn new(options: HttpOptions) -> Self {
        Self {
            options,
            timeout: None,
        }
    }

    pub fn new_with_timeout(options: HttpOptions, timeout: Duration) -> Self {
        Self {
            options,
            timeout: Some(timeout),
        }
    }

    fn agent(&self) -> Result<ureq::Agent, Error> {
        let mut builder = ureq::Agent::config_builder()
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .provider(ureq::tls::TlsProvider::NativeTls)
                    .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                    .build(),
            )
            .user_agent(self.options.user_agent.as_deref().unwrap_or(concat!(
                "proxmox-sync-http-client/",
                env!("CARGO_PKG_VERSION")
            )))
            .timeout_global(self.timeout);

        if let Some(proxy_config) = &self.options.proxy_config {
            builder = builder.proxy(Some(ureq::Proxy::new(&proxy_config.to_proxy_string()?)?));
        }

        Ok(builder.build().into())
    }

    fn add_headers(
        mut req: http::request::Builder,
        content_type: Option<&str>,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> http::request::Builder {
        if let Some(content_type) = content_type {
            req = req.header("Content-Type", content_type);
        }

        if let Some(extra_headers) = extra_headers {
            for (header, value) in extra_headers {
                req = req.header(header, value);
            }
        }

        req
    }

    fn convert_response_to_string(res: Response<ureq::Body>) -> Result<Response<String>, Error> {
        let (parts, mut body) = res.into_parts();
        let body = body.read_to_string()?;
        Ok(Response::from_parts(parts, body))
    }

    fn convert_response_to_vec(res: Response<ureq::Body>) -> Result<Response<Vec<u8>>, Error> {
        let (parts, mut body) = res.into_parts();
        let body = body.read_to_vec()?;
        Ok(Response::from_parts(parts, body))
    }

    fn convert_response_to_reader(res: Response<ureq::Body>) -> Response<Box<dyn Read>> {
        let (parts, body) = res.into_parts();
        Response::from_parts(parts, Box::new(body.into_reader()))
    }
}

impl HttpClient<String, String> for Client {
    fn get(
        &self,
        uri: &str,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<http::Response<String>, Error> {
        let agent = self.agent()?;
        let req = http::Request::get(uri);
        let req = Self::add_headers(req, None, extra_headers);
        let req = req.body(ureq::SendBody::none())?;

        agent
            .run(req)
            .map_err(Error::from)
            .and_then(Self::convert_response_to_string)
    }

    fn post(
        &self,
        uri: &str,
        body: Option<String>,
        content_type: Option<&str>,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<http::Response<String>, Error> {
        let agent = self.agent()?;
        let req = http::Request::post(uri);
        let req = Self::add_headers(req, content_type, extra_headers);

        match body {
            Some(body) => agent.run(req.body(body)?),
            None => agent.run(req.body(ureq::SendBody::none())?),
        }
        .map_err(Error::from)
        .and_then(Self::convert_response_to_string)
    }

    fn request(&self, request: http::Request<String>) -> Result<http::Response<String>, Error> {
        let (parts, body) = request.into_parts();

        let agent = self.agent()?;
        let mut req = http::Request::builder()
            .method(parts.method.as_str())
            .uri(parts.uri);

        for header in parts.headers.keys() {
            for value in parts.headers.get_all(header) {
                req = req.header(header.as_str(), value.to_str()?);
            }
        }

        agent
            .run(req.body(body)?)
            .map_err(Error::from)
            .and_then(Self::convert_response_to_string)
    }
}

impl HttpClient<&[u8], Vec<u8>> for Client {
    fn get(
        &self,
        uri: &str,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<http::Response<Vec<u8>>, Error> {
        let agent = self.agent()?;
        let req = http::Request::get(uri);
        let req = Self::add_headers(req, None, extra_headers);
        let req = req.body(ureq::SendBody::none())?;

        agent
            .run(req)
            .map_err(Error::from)
            .and_then(Self::convert_response_to_vec)
    }

    fn post(
        &self,
        uri: &str,
        body: Option<&[u8]>,
        content_type: Option<&str>,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<http::Response<Vec<u8>>, Error> {
        let agent = self.agent()?;
        let req = http::Request::post(uri);
        let req = Self::add_headers(req, content_type, extra_headers);

        match body {
            Some(body) => agent.run(req.body(body)?),
            None => agent.run(req.body(ureq::SendBody::none())?),
        }
        .map_err(Error::from)
        .and_then(Self::convert_response_to_vec)
    }

    fn request(&self, request: http::Request<&[u8]>) -> Result<http::Response<Vec<u8>>, Error> {
        let (parts, body) = request.into_parts();

        let agent = self.agent()?;
        let mut req = http::Request::builder()
            .method(parts.method.as_str())
            .uri(parts.uri);

        for header in parts.headers.keys() {
            for value in parts.headers.get_all(header) {
                req = req.header(header.as_str(), value.to_str()?);
            }
        }

        agent
            .run(req.body(body)?)
            .map_err(Error::from)
            .and_then(Self::convert_response_to_vec)
    }
}

impl HttpClient<Box<dyn Read>, Box<dyn Read>> for Client {
    fn get(
        &self,
        uri: &str,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<http::Response<Box<dyn Read>>, Error> {
        let agent = self.agent()?;
        let req = http::Request::get(uri);
        let req = Self::add_headers(req, None, extra_headers);
        let req = req.body(ureq::SendBody::none())?;

        agent
            .run(req)
            .map_err(Error::from)
            .map(Self::convert_response_to_reader)
    }

    fn post(
        &self,
        uri: &str,
        body: Option<Box<dyn Read>>,
        content_type: Option<&str>,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<http::Response<Box<dyn Read>>, Error> {
        let agent = self.agent()?;
        let req = http::Request::post(uri);
        let req = Self::add_headers(req, content_type, extra_headers);

        match body {
            Some(body) => agent.run(req.body(ureq::SendBody::from_owned_reader(body))?),
            None => agent.run(req.body(ureq::SendBody::none())?),
        }
        .map_err(Error::from)
        .map(Self::convert_response_to_reader)
    }

    fn request(
        &self,
        request: http::Request<Box<dyn Read>>,
    ) -> Result<http::Response<Box<dyn Read>>, Error> {
        let (parts, body) = request.into_parts();

        let agent = self.agent()?;
        let mut req = http::Request::builder()
            .method(parts.method.as_str())
            .uri(parts.uri);

        for header in parts.headers.keys() {
            for value in parts.headers.get_all(header) {
                req = req.header(header.as_str(), value.to_str()?);
            }
        }

        agent
            .run(req.body(ureq::SendBody::from_owned_reader(body))?)
            .map_err(Error::from)
            .map(Self::convert_response_to_reader)
    }
}
