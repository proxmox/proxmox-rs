use std::env;
use std::sync::Arc;

use http::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use http::method::Method;
use http::status::StatusCode;

use openidconnect::{HttpRequest, HttpResponse};

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
    let mut agent =
        ureq::AgentBuilder::new().tls_connector(Arc::new(native_tls::TlsConnector::new()?));
    if let Ok(val) = env::var("all_proxy").or_else(|_| env::var("ALL_PROXY")) {
        let proxy = ureq::Proxy::new(val).map_err(Box::new)?;
        agent = agent.proxy(proxy);
    }

    Ok(agent.build())
}

///
/// Synchronous HTTP client for ureq.
///
pub fn http_client(request: HttpRequest) -> Result<HttpResponse, Error> {
    let agent = ureq_agent()?;
    let mut req = if let Method::POST = request.method {
        agent.post(request.url.as_ref())
    } else {
        agent.get(request.url.as_ref())
    };

    for (name, value) in request.headers {
        if let Some(name) = name {
            req = req.set(
                name.as_ref(),
                value.to_str().map_err(|_| {
                    Error::Other(format!(
                        "invalid {} header value {:?}",
                        name,
                        value.as_bytes()
                    ))
                })?,
            );
        }
    }

    let response = if let Method::POST = request.method {
        // send_bytes makes sure that Content-Length is set. This is important, because some
        // endpoints don't accept `Transfer-Encoding: chunked`, which would otherwise be set.
        // see https://docs.rs/ureq/2.4.0/ureq/index.html#content-length-and-transfer-encoding
        req.send_bytes(request.body.as_slice())
    } else {
        req.call()
    }
    .map_err(Box::new)?;

    let status_code =
        StatusCode::from_u16(response.status()).map_err(|err| Error::Http(err.into()))?;

    let content_type =
        HeaderValue::from_str(response.content_type()).map_err(|err| Error::Http(err.into()))?;

    Ok(HttpResponse {
        status_code,
        headers: vec![(CONTENT_TYPE, content_type)]
            .into_iter()
            .collect::<HeaderMap>(),
        body: response.into_string()?.as_bytes().into(),
    })
}
