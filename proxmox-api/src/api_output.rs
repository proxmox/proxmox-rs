//! Module to help converting various types into an ApiOutput, mostly required to support 

use serde_json::json;

use super::{ApiOutput, ApiType};

/// Helper trait to convert a variable into an API output.
///
/// If an API method returns a `String`, we want it to be jsonified into `{"data": result}` and
/// wrapped in a `http::Response` with a status code of `200`, but if an API method returns a
/// `http::Response`, we don't want that, our wrappers produced by the `#[api]` macro simply call
/// `output.into_api_output()`, and the trait implementation decides how to proceed.
pub trait IntoApiOutput<T> {
    fn into_api_output(self) -> ApiOutput;
}

impl<T: ApiType + serde::Serialize> IntoApiOutput<()> for T {
    /// By default, any serializable type is serialized into a `{"data": output}` json structure,
    /// and returned as http status 200.
    fn into_api_output(self) -> ApiOutput {
        let output = serde_json::to_value(self)?;
        let res = json!({ "data": output });
        let output = serde_json::to_string(&res)?;
        Ok(http::Response::builder()
            .status(200)
            .header("content-type", "application/json")
            .body(bytes::Bytes::from(output))?
        )
    }
}

/// Methods returning `ApiOutput` (which is a `Result<http::Result<Bytes>, Error>`) don't need
/// anything to happen to the value anymore, return the result as is:
impl IntoApiOutput<ApiOutput> for ApiOutput {
    fn into_api_output(self) -> ApiOutput {
        self
    }
}

/// Methods returning a `http::Response` (without the `Result<_, Error>` around it) need to be
/// wrapped in a `Result`, as we do apply a `?` operator on our methods.
impl IntoApiOutput<ApiOutput> for http::Response<bytes::Bytes> {
    fn into_api_output(self) -> ApiOutput {
        Ok(self)
    }
}
