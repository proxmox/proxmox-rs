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

    check_cli(&cli, &["new", "--foo=FOO", "--bar=BAR"], Ok("FOO:BAR"));
    check_cli(&cli, &["new", "--foo", "FOO", "--bar=BAR"], Ok("FOO:BAR"));
    check_cli(
        &cli,
        &["new", "--foo", "FOO", "--bar", "BAR"],
        Ok("FOO:BAR"),
    );
    check_cli(&cli, &["new", "--foo=FOO"], Err("missing parameter: 'bar'"));
    check_cli(&cli, &["new", "--bar=BAR"], Err("missing parameter: 'foo'"));
    check_cli(&cli, &["new"], Err("missing parameter: 'foo'"));

    check_cli(&cli, &["newfoo", "POSFOO"], Err("missing parameter: 'bar'"));
    check_cli(&cli, &["newfoo", "POSFOO", "--bar=BAR"], Ok("POSFOO:BAR"));
    check_cli(&cli, &["newfoo", "--bar=BAR", "POSFOO"], Ok("POSFOO:BAR"));
    check_cli(
        &cli,
        &["newfoo", "--bar=BAR"],
        Err("missing positional parameters: foo"),
    );

    check_cli(&cli, &["newbar", "POSBAR"], Err("missing parameter: 'foo'"));
    check_cli(&cli, &["newbar", "POSBAR", "--foo=ABC"], Ok("ABC:POSBAR"));
    check_cli(&cli, &["newbar", "--foo=ABC", "POSBAR"], Ok("ABC:POSBAR"));
    check_cli(
        &cli,
        &["newbar", "--foo=ABC"],
        Err("missing positional parameters: bar"),
    );

    check_cli(
        &cli,
        &["newfoo", "FOO1", "--foo=FOO2", "--bar=BAR", "--baz=OMG"],
        Ok("FOO2:BAR:OMG"),
    );

    check_cli(&cli, &["newfoo", "foo", "--bar=b", "--maybe"], Ok("foo:b:[true]"));
    check_cli(&cli, &["newfoo", "foo", "--bar=b", "--maybe=false"], Ok("foo:b:[false]"));
    check_cli(&cli, &["newfoo", "foo", "--bar=b", "--maybe", "false"], Ok("foo:b:[false]"));
}

fn check_cli(cli: &cli::App<Bytes>, args: &[&str], expect: Result<&str, &str>) {
    match (cli.run(args), expect) {
        (Ok(result), Ok(expect)) => {
            let body = std::str::from_utf8(result.body().as_ref())
                .expect("expected a valid utf8 repsonse body");
            assert_eq!(body, expect, "expected successful CLI invocation");
        }
        (Err(result), Err(expected)) => {
            let result = result.to_string();
            assert_eq!(result, expected, "expected specific error message");
        }
        (Ok(result), Err(err)) => match std::str::from_utf8(result.body().as_ref()) {
            Ok(value) => panic!(
                "expected error '{}', got success with value '{}'",
                err, value
            ),
            Err(_) => panic!("expected error '{}', got success with non-utf8 string", err),
        },
        (Err(err), Ok(expected)) => {
            let err = err.to_string();
            panic!(
                "expected success with value '{}', got error '{}'",
                expected, err
            );
        }
    }
}

mod methods {
    use bytes::Bytes;
    use failure::{format_err, Error};
    use http::Response;
    use lazy_static::lazy_static;
    use serde_json::Value;

    use proxmox_api::{get_type_info, ApiFuture, ApiMethod, ApiOutput, ApiType, Parameter};

    fn required_str<'a>(value: &'a Value, name: &'static str) -> Result<&'a str, Error> {
        value[name]
            .as_str()
            .ok_or_else(|| format_err!("missing parameter: '{}'", name))
    }

    pub async fn simple_method(value: Value) -> ApiOutput<Bytes> {
        let foo = required_str(&value, "foo")?;
        let bar = required_str(&value, "bar")?;

        let baz = value
            .get("baz")
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| format_err!("'baz' must be a string"))
            })
            .transpose()?;

        let maybe = value
            .get("maybe")
            .map(|value| {
                value
                    .as_bool()
                    .ok_or_else(|| format_err!("'maybe' must be a boolean, found: {:?}", value))
            })
            .transpose()?;

        let output = match baz {
            Some(baz) => format!("{}:{}:{}", foo, bar, baz),
            None => format!("{}:{}", foo, bar),
        };

        let output = match maybe {
            Some(maybe) => format!("{}:[{}]", output, maybe),
            None => output,
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
                Parameter {
                    name: "maybe",
                    description: "optional boolean test parameter",
                    type_info: Option::<bool>::type_info,
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
