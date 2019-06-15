#![feature(async_await)]

use bytes::Bytes;

use proxmox_api::cli;

#[test]
fn simple() {
    let simple_method: &proxmox_api::ApiMethod<Bytes> = &methods::SIMPLE_METHOD;

    let cli = cli::App::new("simple")
        .subcommand("new", cli::Command::method(simple_method, &[]))
        .subcommand("newfoo", cli::Command::method(simple_method, &["foo"]))
        .subcommand("newbar", cli::Command::method(simple_method, &["bar"]))
        .subcommand(
            "newboth",
            cli::Command::method(simple_method, &["foo", "bar"]),
        );

    let result = cli
        .run(&["new", "--foo=FOO", "--bar=BAR"])
        .expect("command should execute successfully");
    let body =
        std::str::from_utf8(result.body().as_ref()).expect("expected a valid utf8 repsonse body");
    assert_eq!(body, "FOO:BAR");
}

mod methods {
    use bytes::Bytes;
    use http::Response;
    use lazy_static::lazy_static;
    use serde_json::Value;

    use proxmox_api::{get_type_info, ApiFuture, ApiMethod, ApiOutput, ApiType, Parameter};

    pub async fn simple_method(value: Value) -> ApiOutput<Bytes> {
        let foo = value["foo"].as_str().unwrap();

        let bar = value["bar"].as_str().unwrap();

        let baz = value.get("baz").map(|value| value.as_str().unwrap());

        let output = match baz {
            Some(baz) => format!("{}:{}:{}", foo, bar, baz),
            None => format!("{}:{}", foo, bar),
        };

        Ok(Response::builder()
            .status(200)
            .header("content-type", "application/json")
            .body(output.into())?)
    }

    lazy_static! {
        static ref SIMPLE_PARAMS: Vec<Parameter> = {
            vec![
                Parameter {
                    name: "foo",
                    description: "a test parameter",
                    type_info: String::type_info,
                },
                Parameter {
                    name: "bar",
                    description: "another test parameter",
                    type_info: String::type_info,
                },
                Parameter {
                    name: "baz",
                    description: "another test parameter",
                    type_info: Option::<String>::type_info,
                },
            ]
        };
        pub static ref SIMPLE_METHOD: ApiMethod<Bytes> = {
            ApiMethod {
                description: "get some parameters back",
                parameters: &SIMPLE_PARAMS,
                return_type: get_type_info::<String>(),
                protected: false,
                reload_timezone: false,
                handler: |value: Value| -> ApiFuture<Bytes> { Box::pin(simple_method(value)) },
            }
        };
    }
}
