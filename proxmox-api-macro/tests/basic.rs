#![feature(async_await)]

use bytes::Bytes;
use failure::{bail, Error};
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;

use proxmox::api::{api, Router};

#[api({
    description: "A hostname or IP address",
    validate: validate_hostname,
})]
#[derive(Deserialize, Serialize)]
#[repr(transparent)]
pub struct HostOrIp(String);

// We don't bother with the CLI interface in this test:
proxmox::api::no_cli_type! {HostOrIp}

// Simplified for example purposes
fn validate_hostname(name: &str) -> Result<(), Error> {
    if name == "<bad>" {
        bail!("found bad hostname");
    }
    Ok(())
}

#[api({
    description: "A person definition containing name and ID",
    fields: {
        name: {
            description: "The person's full name",
        },
        id: {
            description: "The person's ID number",
            minimum: 1000,
            maximum: 10000,
        },
    },
    cli: false,
})]
#[derive(Deserialize, Serialize)]
pub struct Person {
    name: String,
    id: usize,
}

#[api({
    body: Bytes,
    description: "A test function returning a fixed text",
    parameters: {},
})]
async fn test_body() -> Result<&'static str, Error> {
    Ok("test body")
}

#[api({
    body: Bytes,
    description: "Loopback the `input` parameter",
    parameters: {
        param: "the input",
    },
})]
async fn get_loopback(param: String) -> Result<String, Error> {
    Ok(param)
}

#[api({
    body: Bytes,
    description: "Loopback the `input` parameter",
    parameters: {
        param: "the input",
    },
    returns: String
})]
fn non_async_test(param: String) -> proxmox::api::ApiFuture<Bytes> {
    Box::pin((async move || {
        proxmox::api::IntoApiOutput::into_api_output(param)
    })())
}

proxmox_api_macro::router! {
    static TEST_ROUTER: Router<Bytes> = {
        GET: test_body,

        /subdir: { GET: test_body },
        /subdir/repeated: { GET: test_body },

        /other: { GET: test_body },
        /other/subdir: { GET: test_body },

        /more/{param}: { GET: get_loopback },
        /more/{param}/info: { GET: get_loopback },

        /another/{param}: {
            GET: get_loopback,

            /dir: { GET: non_async_test },
        },

        /wild/{param}*: { GET: get_loopback },
    };
}

fn check_body(router: &Router<Bytes>, path: &str, expect: &'static str) {
    let (router, parameters) = router
        .lookup(path)
        .expect("expected method to exist on test router");
    let method = router
        .get
        .as_ref()
        .expect("expected GET method on router at path");
    let fut = method.call(parameters.unwrap_or(Value::Null));
    let resp = futures::executor::block_on(fut)
        .expect("expected `GET` on test_body to return successfully");
    assert!(resp.status() == 200, "test response should have status 200");
    let body = resp.into_body();
    let body = std::str::from_utf8(&body).expect("expected test body to be valid utf8");
    assert!(
        body == expect,
        "expected test body output to be {:?}, found: {:?}",
        expect,
        body
    );
}

#[test]
fn router() {
    check_body(&TEST_ROUTER, "/subdir", r#"{"data":"test body"}"#);
    check_body(&TEST_ROUTER, "/subdir/repeated", r#"{"data":"test body"}"#);
    check_body(&TEST_ROUTER, "/more/argvalue", r#"{"data":"argvalue"}"#);
    check_body(
        &TEST_ROUTER,
        "/more/argvalue/info",
        r#"{"data":"argvalue"}"#,
    );
    check_body(&TEST_ROUTER, "/another/foo", r#"{"data":"foo"}"#);
    check_body(&TEST_ROUTER, "/another/foo/dir", r#"{"data":"foo"}"#);

    check_body(&TEST_ROUTER, "/wild", r#"{"data":""}"#);
    check_body(&TEST_ROUTER, "/wild/", r#"{"data":""}"#);
    check_body(&TEST_ROUTER, "/wild/asdf", r#"{"data":"asdf"}"#);
    check_body(&TEST_ROUTER, "/wild//asdf", r#"{"data":"asdf"}"#);
    check_body(&TEST_ROUTER, "/wild/asdf/poiu", r#"{"data":"asdf/poiu"}"#);

    // And can I...
    let res = futures::executor::block_on(get_loopback("FOO".to_string()))
        .expect("expected result from get_loopback");
    assert!(
        res == "FOO",
        "expected FOO from direct get_loopback('FOO') call"
    );
}
