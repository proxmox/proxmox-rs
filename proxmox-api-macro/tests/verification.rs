#![feature(async_await)]

use bytes::Bytes;
use failure::{bail, Error};
use serde_json::{json, Value};

use proxmox::api::{api, Router};

#[api({
    body: Bytes,
    description: "A test function returning a fixed text",
    parameters: {
        number: {
            description: "A number",
            minimum: 3,
            maximum: 10,
        },
        reference: {
            description: "A reference number",
            minimum: 3,
            maximum: 10,
        },
    },
})]
async fn less_than(number: usize, reference: usize) -> Result<bool, Error> {
    Ok(number < reference)
}

proxmox_api_macro::router! {
    static TEST_ROUTER: Router<Bytes> = {
        GET: less_than,
    };
}

fn check_parameter(
    router: &Router<Bytes>,
    path: &str,
    parameters: Value,
    expect: Result<&'static str, &'static str>,
) {
    let (router, _) = router
        .lookup(path)
        .expect("expected method to exist on test router");
    let method = router
        .get
        .as_ref()
        .expect("expected GET method on router at path");
    let fut = method.call(parameters);
    match (futures::executor::block_on(fut), expect) {
        (Ok(resp), Ok(exp)) => {
            assert_eq!(resp.status(), 200, "test response should have status 200");
            let body = resp.into_body();
            let body = std::str::from_utf8(&body).expect("expected test body to be valid utf8");
            assert_eq!(body, exp, "expected successful output");
        }
        (Err(resp), Err(exp)) => {
            assert_eq!(resp.to_string(), exp.to_string(), "expected specific error");
        }
        (Ok(resp), Err(exp)) => {
            let body = resp.into_body();
            let body = std::str::from_utf8(&body).expect("expected test body to be valid utf8");
            panic!(
                "expected function to fail with `{}`, but it succeeded with `{}`",
                exp, body
            );
        }
        (Err(resp), Ok(exp)) => {
            panic!(
                "expected function to succeed with `{}`, but it failed with `{}`",
                exp, resp
            );
        }
    }
}

#[test]
fn router() {
    // Expected successes:
    check_parameter(
        &TEST_ROUTER,
        "/",
        json!({
            "number": 3,
            "reference": 5,
        }),
        Ok(r#"{"data":true}"#),
    );

    check_parameter(
        &TEST_ROUTER,
        "/",
        json!({
            "number": 5,
            "reference": 5,
        }),
        Ok(r#"{"data":false}"#),
    );

    // Expected failures:
    check_parameter(
        &TEST_ROUTER,
        "/",
        json!({
            "number": 1,
            "reference": 5,
        }),
        Err("parameter 'number' is out of range (must be >= 3)"),
    );

    check_parameter(
        &TEST_ROUTER,
        "/",
        json!({
            "number": 3,
            "reference": 2,
        }),
        Err("parameter 'reference' is out of range (must be >= 3)"),
    );

    check_parameter(
        &TEST_ROUTER,
        "/",
        json!({
            "number": 3,
            "reference": 20,
        }),
        Err("parameter 'reference' is out of range (must be <= 10)"),
    );

    //// And can I...
    let res = futures::executor::block_on(less_than(1, 5)).map_err(|x| x.to_string());
    assert_eq!(
        res,
        Err("parameter 'number' is out of range (must be >= 3)".to_string()),
        "expected FOO from direct get_loopback('FOO') call"
    );
}
