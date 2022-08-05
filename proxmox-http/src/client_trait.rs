use std::collections::HashMap;

use anyhow::Error;
use http::{Request, Response};

pub trait HttpClient<RequestBody, ResponseBody> {
    fn get(
        &self,
        uri: &str,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<Response<ResponseBody>, Error>;

    fn post(
        &self,
        uri: &str,
        body: Option<RequestBody>,
        content_type: Option<&str>,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<Response<ResponseBody>, Error>;

    fn request(&self, request: Request<RequestBody>) -> Result<Response<ResponseBody>, Error>;
}
