use std::{collections::HashMap, io::Read};

use anyhow::Error;
use http::{Request, Response};

pub trait HttpClient<T> {
    fn get(
        &self,
        uri: &str,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<Response<T>, Error>;

    fn post<R>(
        &self,
        uri: &str,
        body: Option<R>,
        content_type: Option<&str>,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<Response<T>, Error>
    where
        R: Read;

    fn request(&self, request: Request<T>) -> Result<Response<T>, Error>;
}
