use std::env;
use std::io::Read;

use http::method::Method;

use openidconnect::{HttpRequest, HttpResponse};
use ureq::unversioned::transport::Connector;

// Copied from OAuth2 create, because we want to use ureq with
// native-tls. But current OAuth2 crate pulls in rustls, so we cannot
// use their 'ureq' feature.

///
/// Error type returned by failed ureq HTTP requests.
///
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Non-ureq HTTP error.
    #[error("HTTP error - {0}")]
    Http(#[from] http::Error),

    /// IO error
    #[error("IO error - {0}")]
    IO(#[from] std::io::Error),

    /// Error returned by ureq crate.
    // boxed due to https://github.com/algesten/ureq/issues/296
    #[error("ureq request failed - {0}")]
    Ureq(#[from] Box<ureq::Error>),

    #[error("TLS error - {0}")]
    Tls(#[from] native_tls::Error),

    /// Other error.
    #[error("Other error: {0}")]
    Other(String),
}

fn ureq_agent() -> Result<ureq::Agent, Error> {
    let mut config = ureq::Agent::config_builder().tls_config(
        ureq::tls::TlsConfig::builder()
            .provider(ureq::tls::TlsProvider::NativeTls)
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build(),
    );
    if let Ok(val) = env::var("all_proxy").or_else(|_| env::var("ALL_PROXY")) {
        let proxy = ureq::Proxy::new(&val).map_err(Box::new)?;
        config = config.proxy(Some(proxy));
    }
    let agent = ureq::Agent::with_parts(
        config.build(),
        ureq::unversioned::transport::ConnectProxyConnector::default()
            .chain(ureq::unversioned::transport::TcpConnector::default())
            .chain(ureq::unversioned::transport::NativeTlsConnector::default()),
        ureq::unversioned::resolver::DefaultResolver::default(),
    );

    Ok(agent)
}

fn add_headers<T>(
    mut ureq_req: ureq::RequestBuilder<T>,
    oidc_req: &HttpRequest,
) -> Result<ureq::RequestBuilder<T>, Error> {
    for (name, value) in oidc_req.headers() {
        ureq_req = ureq_req.header(
            name,
            value.to_str().map_err(|_| {
                Error::Other(format!(
                    "invalid {} header value {:?}",
                    name,
                    value.as_bytes()
                ))
            })?,
        );
    }
    Ok(ureq_req)
}

///
/// Synchronous HTTP client for ureq.
///
pub fn http_client(request: HttpRequest) -> Result<HttpResponse, Error> {
    let agent = ureq_agent()?;
    let response = if let &Method::POST = request.method() {
        let mut ureq_req = agent.post(request.uri());
        ureq_req = add_headers(ureq_req, &request)?;
        // sending a slice makes sure that Content-Length is set. This is important, because some
        // endpoints don't accept `Transfer-Encoding: chunked`, which would otherwise be set.
        // see https://docs.rs/ureq/3.0.0/ureq/#transfer-encoding-chunked
        ureq_req.send(request.body().as_slice())
    } else {
        let mut ureq_req = agent.get(request.uri());
        ureq_req = add_headers(ureq_req, &request)?;
        ureq_req.call()
    }
    .map_err(Box::new)?;

    let mut bytes: Vec<u8> = Vec::new();

    let (parts, mut body) = response.into_parts();
    body.as_reader().read_to_end(&mut bytes)?;

    Ok(http::Response::from_parts(parts, bytes))
}
