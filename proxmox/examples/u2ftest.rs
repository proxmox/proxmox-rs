//! # live u2f test
//!
//! Listens on `localhost:13905` via http (NOT https) and provides a u2f api test server.
//!
//! To use this, you'll need to create an https wrapper (eg. nginx reverse proxy) with a valid
//! appid, then run:
//!
//! ## Running the API:
//!
//! NOTE: you need to run this in a directory with a `u2f-api.js` file.
//!
//! ```
//! $ cargo run --example u2ftest --features='examples' <APPID>
//! ```
//!
//! Replace `<APPID>` with a working `https://...` url, the API is expected to be on the top level.
//!
//! ## Example Veverse Proxy via nginx:
//!
//! ```
//! server {
//!     listen 443 ssl;
//!     server_name u2ftest.enonet.errno.eu;
//!
//!     ssl_certificate     /etc/nginx/ssl/my.pem;
//!     ssl_certificate_key /etc/nginx/ssl/my.key;
//!     ssl_protocols TLSv1.2;
//!     ssl_ciphers HIGH:!aNULL:!MD5;
//!
//!     root /var/www/html;
//!
//!     location / {
//!         proxy_pass http://127.0.0.1:13905/;
//!     }
//! }
//!
//! ## Debugging
//!
//! The registration and authentication api calls store the response data in
//! `test-registration.json` and `test-auth.json` which get "retried" at startup by the
//! `retry_reg()` and `retry_auth()` calls. This way less hardware interaction is required for
//! debugging.
//! ```

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{bail, format_err, Error};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::stream::StreamExt;

use proxmox::tools::tfa::u2f;

pub const PORT: u16 = 13905;

#[cfg(not(feature = "examples"))]
fn main() {
    let _unused = do_main();
    panic!("rebuild with: --features examples");
}

#[cfg(feature = "examples")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    do_main().await
}

async fn do_main() -> Result<(), Error> {
    use std::net::SocketAddr;
    use std::net::ToSocketAddrs;

    let addr = ("localhost", PORT)
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
        .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], PORT)));

    let appid = std::env::args()
        .skip(1)
        .next()
        .expect("please specify the appid/origin URL");

    let sv = Arc::new(Sv::new(appid));

    // retry the last registration challenge remembered in `test-registration.json`
    sv.retry_reg()?;

    // retry the last authentication challenge remembered in `test-auth.json`
    sv.retry_auth()?;

    let make_service = make_service_fn(move |_conn| {
        let sv = Arc::clone(&sv);
        async move {
            Ok::<_, std::convert::Infallible>(service_fn(move |request: Request<Body>| {
                let sv = Arc::clone(&sv);
                async move {
                    let res = handle(&sv, request).await.or_else(|err| {
                        let err = serde_json::to_string(&serde_json::json!({
                            "error": err.to_string(),
                        }))
                        .unwrap();
                        Ok::<_, std::convert::Infallible>(
                            Response::builder()
                                .status(500)
                                .body(Body::from(err))
                                .unwrap(),
                        )
                    });
                    eprintln!("{:#?}", res);
                    res
                }
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_service);

    server.await?;

    Ok(())
}

async fn fetch_body(mut request_body: Body) -> Result<Vec<u8>, Error> {
    let mut body = Vec::new();

    while let Some(chunk) = request_body.try_next().await? {
        if body.len() + chunk.len() > 1024 * 1024 {
            bail!("request too big");
        }

        body.extend(chunk);
    }

    Ok(body)
}

async fn handle(sv: &Arc<Sv>, request: Request<Body>) -> Result<Response<Body>, Error> {
    let (parts, body) = request.into_parts();
    let body = fetch_body(body).await?;

    eprintln!("fetching: {}", parts.uri.path());
    match parts.uri.path() {
        "/" => simple("text/html", INDEX_HTML),
        "/index.html" => simple("text/html", INDEX_HTML),
        "/u2f-api.js" => file("text/javascript", "u2f-api.js"),
        "/style.css" => simple("text/css", STYLE_CSS),
        "/registration" => sv.registration(),
        "/finish-registration" => sv.finish_registration(serde_json::from_slice(&body)?),
        "/authenticate" => sv.authenticate(serde_json::from_slice(&body)?),
        "/finish-auth" => sv.finish_auth(serde_json::from_slice(&body)?),
        _ => Ok(Response::builder()
            .status(404)
            .body(Body::from("not found"))
            .unwrap()),
    }
}

struct User {
    data: u2f::Registration,
    challenges: HashMap<usize, String>,
}

struct Sv {
    context: u2f::U2f,
    counter: AtomicUsize,
    challenges: Mutex<HashMap<usize, String>>,
    users: Mutex<HashMap<usize, User>>, // key handle
}

impl Sv {
    fn new(appid: String) -> Self {
        Self {
            context: u2f::U2f::new(appid.clone(), appid),
            counter: AtomicUsize::new(0),
            challenges: Mutex::new(HashMap::new()),
            users: Mutex::new(HashMap::new()),
        }
    }

    fn nextid(&self) -> usize {
        self.counter.fetch_add(1, Ordering::AcqRel)
    }

    fn registration(&self) -> Result<Response<Body>, Error> {
        let challenge = self.context.registration_challenge()?;
        let id = self.nextid();
        let output = serde_json::json!({
            "id": id,
            "challenge": challenge,
            "context": &self.context,
        });
        self.challenges
            .lock()
            .unwrap()
            .insert(id, challenge.challenge);
        json(output)
    }

    fn finish_registration(&self, mut response: Value) -> Result<Response<Body>, Error> {
        let id = response["id"]
            .as_u64()
            .ok_or_else(|| format_err!("bad or missing ID in response"))? as usize;
        let rspdata: Value = response
            .as_object_mut()
            .unwrap()
            .remove("response")
            .ok_or_else(|| format_err!("missing response data"))?;

        let challenge = self
            .challenges
            .lock()
            .unwrap()
            .remove(&id)
            .ok_or_else(|| format_err!("no such challenge"))?;

        std::fs::write(
            "test-registration.json",
            serde_json::to_string(&serde_json::json!({
                "challenge": challenge,
                "response": &rspdata,
            }))?,
        )?;

        let data = self
            .context
            .registration_verify_obj(&challenge, serde_json::from_value(rspdata)?)?;

        match data {
            Some(data) => {
                self.users.lock().unwrap().insert(
                    id,
                    User {
                        data,
                        challenges: HashMap::new(),
                    },
                );
                json(serde_json::json!({ "id": id }))
            }
            None => bail!("registration failed"),
        }
    }

    fn retry_reg(&self) -> Result<(), Error> {
        let data = match std::fs::read("test-registration.json") {
            Ok(data) => data,
            Err(ref err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err.into()),
        };

        #[derive(Deserialize)]
        struct TestChallenge {
            challenge: String,
            response: u2f::RegistrationResponse,
        }

        let ts: TestChallenge = serde_json::from_slice(&data)?;

        let res = self
            .context
            .registration_verify_obj(&ts.challenge, ts.response)?;

        eprintln!("=> {:#?}", res);

        Ok(())
    }

    fn authenticate(&self, params: Value) -> Result<Response<Body>, Error> {
        let uid = params["uid"]
            .as_u64()
            .ok_or_else(|| format_err!("bad or missing user id in auth call"))?
            as usize;

        let mut users = self.users.lock().unwrap();
        let user = users
            .get_mut(&uid)
            .ok_or_else(|| format_err!("no such user"))?;

        let challenge = self.context.auth_challenge()?;
        let id = self.nextid();
        user.challenges.insert(id, challenge.challenge.clone());
        let output = serde_json::json!({
            "id": id,
            "challenge": challenge,
            "keys": [&user.data.key],
        });
        json(output)
    }

    fn finish_auth(&self, mut response: Value) -> Result<Response<Body>, Error> {
        let uid = response["uid"]
            .as_u64()
            .ok_or_else(|| format_err!("bad or missing user id in auth call"))?
            as usize;
        let id = response["id"]
            .as_u64()
            .ok_or_else(|| format_err!("bad or missing ID in response"))? as usize;

        let rspdata: Value = response
            .as_object_mut()
            .unwrap()
            .remove("response")
            .ok_or_else(|| format_err!("missing response data"))?;

        let mut users = self.users.lock().unwrap();
        let user = users
            .get_mut(&uid)
            .ok_or_else(|| format_err!("no such user"))?;

        let challenge = user
            .challenges
            .remove(&id)
            .ok_or_else(|| format_err!("no such challenge for user"))?;

        std::fs::write(
            "test-auth.json",
            serde_json::to_string(&serde_json::json!({
                "challenge": challenge,
                "response": &rspdata,
                "user": user.data,
            }))?,
        )?;

        let rspdata: u2f::AuthResponse = serde_json::from_value(rspdata)?;
        if user.data.key.key_handle != rspdata.key_handle() {
            bail!("key handle mismatch");
        }

        let res = self
            .context
            .auth_verify_obj(&user.data.public_key, &challenge, rspdata)?;

        match res {
            Some(auth) => json(serde_json::json!({
                "present": auth.user_present,
                "counter": auth.counter,
            })),
            None => bail!("authentication failed"),
        }
    }

    fn retry_auth(&self) -> Result<(), Error> {
        let data = match std::fs::read("test-auth.json") {
            Ok(data) => data,
            Err(ref err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err.into()),
        };

        #[derive(Deserialize)]
        struct TestChallenge {
            challenge: String,
            user: u2f::Registration,
            response: u2f::AuthResponse,
        }

        let ts: TestChallenge = serde_json::from_slice(&data)?;

        let res = self
            .context
            .auth_verify_obj(&ts.user.public_key, &ts.challenge, ts.response)?;

        eprintln!("=> {:#?}", res);

        Ok(())
    }
}

fn json<T: Serialize>(data: T) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&data)?))
        .unwrap())
}

fn simple(content_type: &'static str, data: &'static str) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(200)
        .header("Content-Type", content_type)
        .body(Body::from(data))
        .unwrap())
}

fn file(content_type: &'static str, file_name: &'static str) -> Result<Response<Body>, Error> {
    let file = std::fs::read(file_name)?;
    Ok(Response::builder()
        .status(200)
        .header("Content-Type", content_type)
        .body(Body::from(file))
        .unwrap())
}

const INDEX_HTML: &str = r##"
<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html>
    <head>
        <link rel="StyleSheet" type="text/css" href="style.css" />
        <script type="text/javascript" src="/u2f-api.js"></script>
        <script type="text/javascript">
            var USER_ID = undefined;

            function clear() {
                document.getElementById("status").innerText = "";
            }

            function log(text) {
                console.log(text);
                document.getElementById("status").innerText += text + "\n";
            }

            function some(x) {
                return x !== undefined && x !== null;
            }

            async function call(path, body) {
                let opts = undefined;
                if (some(body)) {
                    opts = {
                        'method': 'POST',
                        'Content-Type': 'application/json',
                        'body': JSON.stringify(body),
                    };
                }
                return (await fetch(path, opts)).json();
            }

            async function u2f_register(appId, registerRequests, registeredKeys, timeoutSecs) {
                return new Promise((resolve, reject) => {
                    u2f.register(
                        appId,
                        registerRequests,
                        registeredKeys,
                        (rsp) => {
                            if (rsp.errorCode) {
                                reject(rsp);
                            } else {
                                delete rsp.errorCode;
                                resolve(rsp);
                            }
                        },
                        timeoutSecs,
                    );
                });
            }

            async function u2f_sign(appId, challenge, registeredKeys, timeoutSecs) {
                return new Promise((resolve, reject) => {
                    u2f.sign(
                        appId,
                        challenge,
                        registeredKeys,
                        (rsp) => {
                            if (rsp.errorCode) {
                                reject(rsp);
                            } else {
                                delete rsp.errorCode;
                                resolve(rsp);
                            }
                        },
                        timeoutSecs,
                    );
                });
            }

            async function register() {
                try {
                    clear();

                    log("fetching registration");
                    let registration = await call("/registration");

                    log("logging registration challenge to console");
                    console.log(registration);
                    let data = registration.challenge;
                    let challenge = {
                        "challenge": data.challenge,
                        "version": data.version,
                    };

                    log("please press the button on your u2f token");
                    let rsp = await u2f_register(
                        data.appId,
                        [challenge],
                        [],
                        10,
                    );

                    log("logging response to console");
                    console.log(rsp);

                    log("replying");
                    let reg = await call("/finish-registration", {
                        'response': rsp,
                        'id': registration.id,
                    });
                    log("server responded");
                    log(JSON.stringify(reg));

                    let id = reg.id;
                    log("Our user id is now: " + id);
                    USER_ID = id;
                } catch (ex) {
                    log("An exception occurred:");
                    console.log(ex);
                    log(ex);
                }
            }

            async function authenticate() {
                try {
                    clear();

                    if (!some(USER_ID)) {
                        log("not authenticated");
                        return;
                    }

                    log("fetching authentication");
                    let auth = await call("/authenticate", {
                        'uid': USER_ID,
                    });

                    log("logging authentication");
                    console.log(auth);

                    log("please press the button on your u2f token");
                    let rsp = await u2f_sign(
                        auth.challenge.appId,
                        auth.challenge.challenge,
                        auth.keys,
                        10,
                    );

                    log("logging token to console");
                    console.log(rsp);

                    log("replying");
                    let reg = await call("/finish-auth", {
                        'response': rsp,
                        'uid': USER_ID,
                        'id': auth.id,
                    });
                    log("server responded");
                    log(JSON.stringify(reg));
                } catch (ex) {
                    log("An exception occurred:");
                    console.log(ex);
                    log(ex);
                }
            }
        </script>
    </head>
    <body>
        <div id="buttons">
            <a href="#" onclick="register();">Register</a>
            <a href="#" onclick="authenticate();">Authenticate</a>
        </div>
        <div id="status">
            Select an action.
        </div>
    </body>
</html>
"##;

const STYLE_CSS: &str = r##"
body {
    background-color: #f8fff8;
    padding: 3em 10em 3em 10em;
}

p, a, h1, h2, h3, h4 {
    margin: 0;
    padding: 0;
}

#status {
    background-color: #fff;
    border: 1px solid #ccc;
    margin: auto;
    width: 80em;
    font-family: monospace;
}

"##;
