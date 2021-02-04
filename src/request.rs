use serde::Deserialize;

pub const JSON_CONTENT_TYPE: &str = "application/jose+json";

pub const CREATED: u16 = 201;

/// A request which should be performed on the ACME provider.
pub struct Request {
    pub url: String,
    pub method: &'static str,
    pub content_type: &'static str,
    pub body: String,

    pub expected: u16,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ErrorResponse {
    #[serde(rename = "type")]
    pub ty: String,
    pub detail: Option<String>,
    pub subproblems: Option<serde_json::Value>,
}
