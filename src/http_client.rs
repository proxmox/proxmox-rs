use http::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use http::method::Method;
use http::status::StatusCode;

use openidconnect::{
    HttpRequest,
    HttpResponse,
};

// Copied from OAuth2 create, because we want to use ureq with
// native-tls. But current OAuth2 crate pulls in rustls, so we cannot
// use their 'ureq' feature.

///
/// Error type returned by failed ureq HTTP requests.
///
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Non-ureq HTTP error.
    #[error("HTTP error")]
    Http(#[from] http::Error),
    /// IO error
    #[error("IO error")]
    IO(#[from] std::io::Error),
    /// Other error.
    #[error("Other error: {}", _0)]
    Other(String),
    /// Error returned by ureq crate.
    // boxed due to https://github.com/algesten/ureq/issues/296
    #[error("ureq request failed")]
    Ureq(#[from] Box<ureq::Error>),
}

///
/// Synchronous HTTP client for ureq.
///
pub fn http_client(request: HttpRequest) -> Result<HttpResponse, Error> {
   let mut req = if let Method::POST = request.method {
        ureq::post(&request.url.to_string())
    } else {
        ureq::get(&request.url.to_string())
    };

    for (name, value) in request.headers {
        if let Some(name) = name {
            req = req.set(
                &name.to_string(),
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
        req.send(&*request.body)
    } else {
        req.call()
    }
    .map_err(Box::new)?;

    let status_code = StatusCode::from_u16(response.status())
        .map_err(|err| Error::Http(err.into()))?;

    let content_type = HeaderValue::from_str(response.content_type())
        .map_err(|err| Error::Http(err.into()))?;

    Ok(HttpResponse {
        status_code,
        headers: vec![(CONTENT_TYPE, content_type)]
            .into_iter()
            .collect::<HeaderMap>(),
        body: response.into_string()?.as_bytes().into(),
    })
}
