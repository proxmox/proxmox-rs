use anyhow::Error;
use serde_json::Value;

use proxmox_router::cli::{CliCommand, CliCommandMap, CommandLineInterface, GlobalOptions};
use proxmox_router::{ApiHandler, ApiMethod, RpcEnvironment};
use proxmox_schema::format::DocumentationFormat;
use proxmox_schema::{
    ApiStringFormat, ApiType, BooleanSchema, EnumEntry, ObjectSchema, Schema, StringSchema,
};

fn dummy_method(
    _param: Value,
    _info: &ApiMethod,
    _rpcenv: &mut dyn RpcEnvironment,
) -> Result<Value, Error> {
    Ok(Value::Null)
}

const API_METHOD_SIMPLE1: ApiMethod = ApiMethod::new(
    &ApiHandler::Sync(&dummy_method),
    &ObjectSchema::new(
        "Simple API method with one required and one optional argument.",
        &[
            (
                "another-required-arg",
                false,
                &StringSchema::new("A second required string argument.").schema(),
            ),
            (
                "optional-arg",
                true,
                &BooleanSchema::new("Optional boolean argument.")
                    .default(false)
                    .schema(),
            ),
            (
                "required-arg",
                false,
                &StringSchema::new("Required string argument.").schema(),
            ),
        ],
    ),
);

#[allow(dead_code)]
struct GlobalOpts {
    global: String,
}

impl<'de> serde::Deserialize<'de> for GlobalOpts {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        unreachable!("not used in tests, implemented to satisfy `.global_option` constraint");
    }
}

impl ApiType for GlobalOpts {
    const API_SCHEMA: Schema = ObjectSchema::new(
        "Global options.",
        &[
            (
                "global1",
                true,
                &StringSchema::new("A global option.")
                    .format(&ApiStringFormat::Enum(&[
                        EnumEntry::new("one", "Option one."),
                        EnumEntry::new("two", "Option two."),
                    ]))
                    .schema(),
            ),
            (
                "global2",
                true,
                &StringSchema::new("A second global option.").schema(),
            ),
        ],
    )
    .schema();
}

/// Generates the following:
///
/// ```text
/// clicmd l0c1 --required-arg --another-required-arg [--optional-arg]
/// clicmd l0c2 <required-arg> --another-required-arg [--optional-arg]
/// clicmd l0sub l1c1 --required-arg --another-required-arg [--optional-arg]
/// clicmd l0sub l1c2 --required-arg --another-required-arg [--optional-arg]
/// ```
fn get_complex_test_cmddef() -> CliCommandMap {
    let sub_def = CliCommandMap::new()
        .global_option(GlobalOptions::of::<GlobalOpts>())
        .insert("l1c1", CliCommand::new(&API_METHOD_SIMPLE1))
        .insert("l1c2", CliCommand::new(&API_METHOD_SIMPLE1));

    CliCommandMap::new()
        .insert_help()
        .insert("l0sub", CommandLineInterface::Nested(sub_def))
        .insert("l0c1", CliCommand::new(&API_METHOD_SIMPLE1))
        .insert(
            "l0c2",
            CliCommand::new(&API_METHOD_SIMPLE1).arg_param(&["required-arg"]),
        )
}

fn expected_toplevel_help_text() -> &'static str {
    r##"
Usage:

clicmd help [{<command>}] [OPTIONS]
clicmd l0c1 --another-required-arg <string> --required-arg <string> [OPTIONS]
clicmd l0c2 <required-arg> --another-required-arg <string> [OPTIONS]
clicmd l0sub l1c1 --another-required-arg <string> --required-arg <string> [OPTIONS]
clicmd l0sub l1c2 --another-required-arg <string> --required-arg <string> [OPTIONS]
"##
    .trim_start()
}

fn expected_group_help_text() -> &'static str {
    r##"
Usage: clicmd l0sub l1c1 --another-required-arg <string> --required-arg <string> [OPTIONS]

Simple API method with one required and one optional argument.

 --another-required-arg <string>
             A second required string argument.

 --required-arg <string>
             Required string argument.

Optional parameters:

 --optional-arg <boolean>   (default=false)
             Optional boolean argument.

Inherited group parameters:

 --global1  one|two
             A global option.
 --global2  <string>
             A second global option.
"##
    .trim_start()
}

fn expected_nested_usage_text() -> &'static str {
    r##"
``clicmd help [{<command>}] [OPTIONS]``

Get help about specified command (or sub-command).

``<command>`` : ``<string>``
  Command. This may be a list in order to specify nested sub-commands. Can be
  specified more than once.

Optional parameters:

``--verbose`` ``<boolean>``
  Verbose help.

----

``clicmd l0c1 --another-required-arg <string> --required-arg <string> [OPTIONS]``

Simple API method with one required and one optional argument.

``--another-required-arg`` ``<string>``
  A second required string argument.

``--required-arg`` ``<string>``
  Required string argument.

Optional parameters:

``--optional-arg`` ``<boolean>   (default=false)``
  Optional boolean argument.

----

``clicmd l0c2 <required-arg> --another-required-arg <string> [OPTIONS]``

Simple API method with one required and one optional argument.

``<required-arg>`` : ``<string>``
  Required string argument.

``--another-required-arg`` ``<string>``
  A second required string argument.

Optional parameters:

``--optional-arg`` ``<boolean>   (default=false)``
  Optional boolean argument.

----

Options available for command group ``clicmd l0sub``:

``--global1`` ``one|two``
  A global option.

``--global2`` ``<string>``
  A second global option.

----

``clicmd l0sub l1c1 --another-required-arg <string> --required-arg <string> [OPTIONS]``

Simple API method with one required and one optional argument.

``--another-required-arg`` ``<string>``
  A second required string argument.

``--required-arg`` ``<string>``
  Required string argument.

Optional parameters:

``--optional-arg`` ``<boolean>   (default=false)``
  Optional boolean argument.

Inherited group parameters:

``--global1``

``--global2``

----

``clicmd l0sub l1c2 --another-required-arg <string> --required-arg <string> [OPTIONS]``

Simple API method with one required and one optional argument.

``--another-required-arg`` ``<string>``
  A second required string argument.

``--required-arg`` ``<string>``
  Required string argument.

Optional parameters:

``--optional-arg`` ``<boolean>   (default=false)``
  Optional boolean argument.

Inherited group parameters:

``--global1``

``--global2``"##
        .trim_start()
}

#[test]
fn test_nested_usage() {
    let doc = proxmox_router::cli::generate_nested_usage(
        "clicmd",
        &get_complex_test_cmddef(),
        DocumentationFormat::ReST,
    );
    // println!("--- BEGIN EXPECTED DOC OUTPUT ---");
    // print!("{doc}");
    // println!("--- END EXPECTED DOC OUTPUT ---");
    assert_eq!(doc, expected_nested_usage_text());
}

#[test]
fn test_toplevel_help() {
    let mut help = String::new();
    proxmox_router::cli::print_help_to(
        &get_complex_test_cmddef().into(),
        "clicmd".to_string(),
        &[],
        None,
        &mut help,
    )
    .expect("failed to format help string");
    // println!("--- BEGIN EXPECTED DOC OUTPUT ---");
    // print!("{help}");
    // println!("--- END EXPECTED DOC OUTPUT ---");
    assert_eq!(help, expected_toplevel_help_text());
}

#[test]
fn test_group_help() {
    let mut help = String::new();
    proxmox_router::cli::print_help_to(
        &get_complex_test_cmddef().into(),
        "clicmd".to_string(),
        &["l0sub".to_string(), "l1c1".to_string()],
        None,
        &mut help,
    )
    .expect("failed to format help string");
    // println!("--- BEGIN EXPECTED DOC OUTPUT ---");
    // print!("{help}");
    // println!("--- END EXPECTED DOC OUTPUT ---");
    assert_eq!(help, expected_group_help_text());
}
