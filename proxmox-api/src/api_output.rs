//! Module to help converting various types into an ApiOutput, mostly required to support 

use serde_json::json;

use super::{ApiOutput, ApiType};

pub trait IntoApiOutput<T> {
    fn into_api_output(self) -> ApiOutput;
}

impl<T: ApiType + serde::Serialize> IntoApiOutput<()> for T {
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

impl IntoApiOutput<ApiOutput> for ApiOutput {
    fn into_api_output(self) -> ApiOutput {
        self
    }
}

impl IntoApiOutput<ApiOutput> for http::Response<bytes::Bytes> {
    fn into_api_output(self) -> ApiOutput {
        Ok(self)
    }
}
