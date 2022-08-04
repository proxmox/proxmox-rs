use std::collections::HashMap;

use anyhow::Error;
use http::{Request, Response};

pub trait HttpClient<T> {
    fn get(
        &self,
        uri: &str,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<Response<T>, Error>;

    fn post(
        &self,
        uri: &str,
        body: Option<&str>,
        content_type: Option<&str>,
    ) -> Result<Response<T>, Error>;

    fn request(&self, request: Request<T>) -> Result<Response<T>, Error>;
}
