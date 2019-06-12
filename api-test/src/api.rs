use std::io;
use std::path::Path;

use failure::{bail, Error};
use futures::compat::AsyncRead01CompatExt;
use futures::compat::Future01CompatExt;
use futures::io::AsyncReadExt;
use http::Response;
use hyper::Body;

use proxmox::api::{api, router};

#[api({
    description: "Hello API call",
})]
async fn hello() -> Result<Response<Body>, Error> {
    Ok(http::Response::builder()
        .status(200)
        .header("content-type", "text/html")
        .body(Body::from("Hello"))?)
}

static mut WWW_DIR: Option<String> = None;

pub fn www_dir() -> &'static str {
    unsafe {
        WWW_DIR
            .as_ref()
            .expect("expected WWW_DIR to be initialized")
            .as_str()
    }
}

pub fn set_www_dir(dir: String) {
    unsafe {
        assert!(WWW_DIR.is_none(), "WWW_DIR must only be initialized once!");

        WWW_DIR = Some(dir);
    }
}

#[api({
    description: "Get a file from the www/ subdirectory.",
    parameters: {
        path: "Path to the file to fetch",
    },
})]
async fn get_www(path: String) -> Result<Response<Body>, Error> {
    if path.contains("..") {
        bail!("illegal path");
    }

    let mut file = match tokio::fs::File::open(format!("{}/{}", www_dir(), path))
        .compat()
        .await
    {
        Ok(file) => file,
        Err(ref err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(http::Response::builder()
                .status(404)
                .body(Body::from(format!("No such file or directory: {}", path)))?);
        }
        Err(e) => return Err(e.into()),
    }
    .compat();

    let mut data = Vec::new();
    file.read_to_end(&mut data).await?;

    let mut response = http::Response::builder();
    response.status(200);

    let content_type = match Path::new(&path).extension().and_then(|e| e.to_str()) {
        Some("html") => Some("text/html"),
        Some("css") => Some("text/css"),
        Some("js") => Some("application/javascript"),
        Some("txt") => Some("text/plain"),
        // ...
        _ => None,
    };
    if let Some(content_type) = content_type {
        response.header("content-type", content_type);
    }

    Ok(response.body(Body::from(data))?)
}

router! {
    pub static ROUTER: Router<Body> = {
        GET: hello,
        /www/{path}*: { GET: get_www },
        /api/1: {
        }
    };
}
