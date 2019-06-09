//! Module to help converting various types into an ApiOutput, mostly required to support

use serde_json::json;

use super::{ApiOutput, ApiType};

/// Helper trait to convert a variable into an API output.
///
/// If an API method returns a `String`, we want it to be jsonified into `{"data": result}` and
/// wrapped in a `http::Response` with a status code of `200`, but if an API method returns a
/// `http::Response`, we don't want that, our wrappers produced by the `#[api]` macro simply call
/// `output.into_api_output()`, and the trait implementation decides how to proceed.
pub trait IntoApiOutput<Body, T> {
    fn into_api_output(self) -> ApiOutput<Body>;
}

impl<Body, T> IntoApiOutput<Body, ()> for T
where
    Body: 'static,
    T: ApiType + serde::Serialize,
    Body: From<String>,
{
    /// By default, any serializable type is serialized into a `{"data": output}` json structure,
    /// and returned as http status 200.
    fn into_api_output(self) -> ApiOutput<Body> {
        let output = serde_json::to_value(self)?;
        let res = json!({ "data": output });
        let output = serde_json::to_string(&res)?;
        Ok(http::Response::builder()
            .status(200)
            .header("content-type", "application/json")
            .body(Body::from(output))?)
    }
}

/// Methods returning `ApiOutput` (which is a `Result<http::Result<Bytes>, Error>`) don't need
/// anything to happen to the value anymore, return the result as is:
impl<Body> IntoApiOutput<Body, ApiOutput<Body>> for ApiOutput<Body> {
    fn into_api_output(self) -> ApiOutput<Body> {
        self
    }
}

/// Methods returning a `http::Response` (without the `Result<_, Error>` around it) need to be
/// wrapped in a `Result`, as we do apply a `?` operator on our methods.
impl<Body> IntoApiOutput<Body, ApiOutput<Body>> for http::Response<Body> {
    fn into_api_output(self) -> ApiOutput<Body> {
        Ok(self)
    }
}
