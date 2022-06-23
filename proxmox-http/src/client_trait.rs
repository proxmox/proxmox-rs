use anyhow::Error;
use http::Response;

pub trait HttpClient<T> {
    fn get(&self, uri: &str) -> Result<Response<T>, Error>;

    fn post(
        &self,
        uri: &str,
        body: Option<&str>,
        content_type: Option<&str>,
    ) -> Result<Response<T>, Error>;
}
