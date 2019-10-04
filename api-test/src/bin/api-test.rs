use std::io;
use std::path::Path;

use failure::{bail, format_err, Error};
use http::Request;
use http::Response;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Server};
use serde_json::Value;
use tokio::io::AsyncReadExt;

use proxmox::api::{api, router};

//
// Configuration:
//

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

//
// Complex types allowed in the API
//

#[api({
    description: "A test enum",
})]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MountType {
    Volume,
    BindMount,
    #[api(rename = "pass-through-device")]
    PassThrough,
}

#[api({
    description: "A test struct",
    cli: false, // no CLI interface for now...
    fields: {
        mount_type: "The type of mount point",
        source: "The path to mount",
        destination: {
            description: "Target path to mount at",
            pattern: r#"^[^.]"#, // must not start with a dot
        },
        ro: {
            description: "Whether to mount read-only",
            default: false,
        },
    },
})]
#[derive(Debug)]
pub struct MountEntry {
    mount_type: MountType,
    source: String,
    destination: String,
    ro: Option<bool>,
}

//
// API methods
//

router! {
    pub static ROUTER: Router<Body> = {
        GET: hello,
        /www/{path}*: { GET: get_www },
        /api/1: {
            /greet: { GET: greet_person_with },
            /mount/{id}: { POST: update_mount_point },
        }
    };
}

#[api({
    description: "Hello API call",
})]
async fn hello() -> Result<Response<Body>, Error> {
    Ok(http::Response::builder()
        .status(200)
        .header("content-type", "text/html")
        .body(Body::from("Hello"))?)
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

    // FIXME: Add support for an ApiError type for 404s etc. to reduce error handling code size:
    // Compiler bug: cannot use format!() in await expressions...
    let file_path = format!("{}/{}", www_dir(), path);
    let mut file = match tokio::fs::File::open(&file_path).await {
        Ok(file) => file,
        Err(ref err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(http::Response::builder()
                .status(404)
                .body(Body::from(format!("No such file or directory: {}", path)))?);
        }
        Err(e) => return Err(e.into()),
    };

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

#[api({
    description: "Create a greeting message with various parameters...",
    parameters: {
        person: "The person to greet",
        message: "The message to give",
        ps: "An optional PS message",
    },
})]
async fn greet_person_with(
    person: String,
    message: String,
    ps: Option<String>,
) -> Result<String, Error> {
    Ok(match ps {
        Some(ps) => format!("{}, {}.\n{}", person, message, ps),
        None => format!("{}, {}.", person, message),
    })
}

#[api({
    description: "Update or create the configuration for a mount point",
    parameters: {
        id: "Which mount point entry to configure",
        entry: "The mount point configuration to replace the entry with",
    },
})]
async fn update_mount_point(id: String, entry: MountEntry) -> Result<String, Error> {
    eprintln!("Got request to update mount point '{}'", id);
    eprintln!("New configuration: {:?}", entry);
    Ok(format!("Updating '{}' with: {:?}", id, entry))
}

//
// Hyper glue
//

async fn json_body(mut body: Body) -> Result<Value, Error> {
    let mut data = Vec::new();
    while let Some(chunk) = body.next().await {
        data.extend(chunk?);
    }
    Ok(serde_json::from_str(std::str::from_utf8(&data)?)?)
}

async fn route_request(request: Request<Body>) -> Result<http::Response<Body>, Error> {
    let (parts, body) = request.into_parts();
    let path = parts.uri.path();

    let (target, mut params) = ROUTER
        .lookup(path)
        .ok_or_else(|| format_err!("missing path: {}", path))?;

    use hyper::Method;
    let method = match parts.method {
        Method::GET => target.get.as_ref(),
        Method::PUT => target.put.as_ref(),
        Method::POST => target.post.as_ref(),
        Method::DELETE => target.delete.as_ref(),
        _ => bail!("unexpected method type"),
    };

    if let Some(ty) = parts.headers.get(http::header::CONTENT_TYPE) {
        if ty.to_str()? == "application/json" {
            let json = json_body(body).await?;
            match json {
                Value::Object(map) => {
                    for (k, v) in map {
                        let existed = params
                            .get_or_insert_with(serde_json::Map::new)
                            .insert(k, v)
                            .is_some();
                        if existed {
                            bail!("tried to override path-based parameter!");
                        }
                    }
                }
                _ => bail!("expected a json object"),
            }
        }
    }

    method
        .ok_or_else(|| format_err!("no {:?} method found for: {}", parts.method, path))?
        .call(params.map(Value::Object).unwrap_or(Value::Null))
        .await
}

async fn service_func(request: Request<Body>) -> Result<http::Response<Body>, hyper::Error> {
    match route_request(request).await {
        Ok(r) => Ok(r),
        Err(err) => Ok(Response::builder()
            .status(400)
            .body(Body::from(format!("ERROR: {}", err)))
            .expect("building an error response...")),
    }
}

//
// Main entry point
//
async fn main_do(www_dir: String) {
    // Construct our SocketAddr to listen on...
    let addr = ([0, 0, 0, 0], 3000).into();

    // And a MakeService to handle each connection...
    let service = make_service_fn(|_| async { Ok::<_, hyper::Error>(service_fn(service_func)) });

    // Then bind and serve...
    let server = Server::bind(&addr).serve(service);

    println!("Serving {} under http://localhost:3000/www/", www_dir);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}

fn main() {
    // We expect a path, where to find our files we expose via the www/ dir:
    let mut args = std::env::args();

    // real code should have better error handling
    let _program_name = args.next();
    let www_dir = args.next().expect("expected a www/ subdirectory");
    set_www_dir(www_dir.to_string());

    // show our api info:
    println!(
        "{}",
        serde_json::to_string_pretty(&ROUTER.api_dump()).unwrap()
    );

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(main_do(www_dir));
}
