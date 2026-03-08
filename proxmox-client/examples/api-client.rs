//! Minimal REST client for testing `proxmox-client` connectivity and error handling.
//!
//! Connects to a Proxmox API endpoint, authenticates, and performs HTTP requests.  Prints the error
//! variant on failure, which is useful for verifying `Connect` vs `Client` classification.
//!
//! # Usage
//!
//! ```text
//! cargo run -p proxmox-client --features hyper-client --example api-client -- [OPTIONS] URL
//!
//! Options:
//!   --fingerprint HEX   Expected TLS certificate fingerprint (SHA-256)
//!   --insecure          Skip TLS verification
//!   --user USER         Authenticate with username (prompts for password)
//!   --token USER!NAME   Authenticate with API token (prompts for secret)
//!   --method METHOD     HTTP method (default: GET)
//!   --path PATH         API path (default: /api2/json/version)
//!   --data JSON         JSON request body
//! ```
//!
//! Secrets can also be passed via environment variables instead of interactive
//! prompts:
//!
//! - `PROXMOX_PASSWORD` — password for `--user`
//! - `PROXMOX_TOKEN_SECRET` — secret for `--token`
//!
//! # Examples
//!
//! ```text
//! # Simple version query:
//! api-client --insecure --user root@pam https://pve:8006
//!
//! # With password from environment:
//! PROXMOX_PASSWORD=secret api-client --insecure --user root@pam https://pve:8006
//!
//! # Start a VM:
//! api-client --insecure --token admin@pam!test https://pve:8006 \
//!     --method POST --path /api2/json/nodes/node1/qemu/100/status/start
//!
//! # Update a configuration value:
//! api-client --insecure --token admin@pam!test https://pve:8006 \
//!     --method PUT --path /api2/json/nodes/node1/qemu/100/config \
//!     --data '{"memory": 4096}'
//! ```

use http::Method;
use proxmox_client::{Client, Error, HttpApiClient, TlsOptions, Token};

struct Args {
    url: String,
    tls: TlsOptions,
    user: Option<String>,
    token: Option<String>,
    method: Method,
    path: String,
    data: Option<serde_json::Value>,
}

fn parse_args() -> Args {
    let mut args = std::env::args().skip(1);
    let mut tls = TlsOptions::Verify;
    let mut user = None;
    let mut token = None;
    let mut method = Method::GET;
    let mut url = None;
    let mut path = "/api2/json/version".to_string();
    let mut data = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--fingerprint" => {
                let fp = args.next().expect("--fingerprint requires a value");
                tls = TlsOptions::parse_fingerprint(&fp).expect("invalid fingerprint");
            }
            "--insecure" => tls = TlsOptions::Insecure,
            "--user" => user = Some(args.next().expect("--user requires a value")),
            "--token" => token = Some(args.next().expect("--token requires USER!NAME")),
            "--method" => {
                let m = args.next().expect("--method requires a value");
                method = m
                    .parse()
                    .unwrap_or_else(|_| panic!("invalid HTTP method: {m}"));
            }
            "--path" => path = args.next().expect("--path requires a value"),
            "--data" => {
                let raw = args.next().expect("--data requires a JSON value");
                data = Some(
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("invalid JSON: {e}")),
                );
            }
            s if s.starts_with('-') => {
                eprintln!("unknown option: {s}");
                std::process::exit(2);
            }
            _ => url = Some(arg),
        }
    }

    Args {
        url: url.expect("URL argument required"),
        tls,
        user,
        token,
        method,
        path,
        data,
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = parse_args();

    let uri = args.url.parse().expect("invalid URL");
    let mut client =
        Client::with_options(uri, args.tls, Default::default()).expect("failed to create client");
    client.set_pve_compatibility(true);

    if let Some(token_id) = &args.token {
        let secret = secret_from_env_or_prompt("PROXMOX_TOKEN_SECRET", "Token secret: ");
        client.set_authentication(Token {
            userid: token_id.clone(),
            prefix: "PVEAPIToken".to_string(),
            value: secret,
            perl_compat: true,
        });
        eprintln!("auth: token set");
    } else if let Some(user) = &args.user {
        let password = secret_from_env_or_prompt("PROXMOX_PASSWORD", "Password: ");

        // Login::new() expects the server base URL (it appends /api2/json/access/ticket).
        let api_url = client.api_url();
        let server_url = format!(
            "{}://{}",
            api_url.scheme_str().unwrap_or("https"),
            api_url.authority().map(|a| a.as_str()).unwrap_or(""),
        );
        let login = proxmox_login::Login::new(server_url, user.clone(), password);
        eprintln!("login: POST {}", login.request().url);

        match client.login(login).await {
            Ok(None) => eprintln!("auth: login ok"),
            Ok(Some(tfa)) => {
                print_error(&Error::TfaRequired(tfa.challenge));
                std::process::exit(1);
            }
            Err(err) => {
                print_error(&err);
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("auth: none (use --user or --token to authenticate)");
    }

    eprintln!("{} {}", args.method, args.path);
    let result = client
        .request(args.method, &args.path, args.data.as_ref())
        .await;

    match result {
        Ok(response) => {
            eprintln!("status: {}", response.status);
            if let Ok(text) = String::from_utf8(response.body) {
                println!("{text}");
            } else {
                eprintln!("(binary response body)");
            }
        }
        Err(err) => {
            print_error(&err);
            std::process::exit(1);
        }
    }
}

fn print_error(err: &Error) {
    let variant = match err {
        Error::Unauthorized => "Unauthorized",
        Error::Api(..) => "Api",
        Error::BadApi(..) => "BadApi",
        Error::UnexpectedData => "UnexpectedData",
        Error::Authentication(..) => "Authentication",
        Error::Ticket(..) => "Ticket",
        Error::Other(..) => "Other",
        Error::Connect(..) => "Connect",
        Error::Client(..) => "Client",
        Error::TfaRequired(..) => "TfaRequired",
        Error::Internal(..) => "Internal",
        Error::Anyhow(..) => "Anyhow",
        _ => "Unknown",
    };

    eprintln!("error [{variant}]: {err}");

    use std::error::Error as _;
    let mut source = err.source();
    while let Some(cause) = source {
        eprintln!("  caused by: {cause}");
        source = cause.source();
    }
}

fn secret_from_env_or_prompt(env_var: &str, prompt: &str) -> String {
    if let Ok(val) = std::env::var(env_var) {
        return val;
    }
    let bytes = proxmox_sys::linux::tty::read_password(prompt).expect("failed to read secret");
    String::from_utf8(bytes).expect("secret is not valid UTF-8")
}
