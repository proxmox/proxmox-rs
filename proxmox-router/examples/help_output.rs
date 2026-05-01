//! Demonstrates the help output produced by [`CommandLine`] for a typical CLI shape: one
//! [`GlobalOptions`] struct registered on the root, one nested subgroup, one leaf command.
//! Run it to see what an end user encounters.
//!
//! ```text
//! $ cargo run --example help_output -- help mirror snapshot create
//! Usage: mirror snapshot create <id> [OPTIONS]
//!
//! Create a new repository snapshot.
//!
//!  <id>       <string>
//!              Mirror name.
//!
//! Optional parameters:
//!
//!  --dry-run  <boolean>   (default=false)
//!              Print what would happen.
//!
//! Inherited group parameters:
//!
//!  --config   <string>
//!              Path to mirroring config file.
//! ```
//!
//! Other invocations to try:
//! - `cargo run --example help_output --` (no args) prints the nested-usage banner with a
//!   `Global options for `help_output`: --config <string>` line at the top, followed by the
//!   subcommand list (`help`, `mirror snapshot create`).
//! - `cargo run --example help_output -- mirror snapshot create my-mirror` actually runs
//!   the dummy handler.
//!
//! Notes for readers copying this example:
//! - The literal `help_output` strings in the output above come from cargo's example name;
//!   a real binary shows its own argv[0] instead.
//! - The `Usage:` line of the `help <subcommand>` output omits the binary prefix because
//!   the built-in `help` command constructs its prefix that way; this is unrelated to the
//!   global-options handling shown here.

use anyhow::Error;
use serde::Deserialize;
use serde_json::Value;

use proxmox_router::cli::{
    CliCommand, CliCommandMap, CommandLine, CommandLineInterface, GlobalOptions,
};
use proxmox_router::{ApiHandler, ApiMethod, RpcEnvironment};
use proxmox_schema::{ApiType, BooleanSchema, ObjectSchema, Schema, StringSchema};

fn create_snapshot(_: Value, _: &ApiMethod, _: &mut dyn RpcEnvironment) -> Result<Value, Error> {
    println!("(would create a snapshot here)");
    Ok(Value::Null)
}

const API_METHOD_CREATE: ApiMethod = ApiMethod::new(
    &ApiHandler::Sync(&create_snapshot),
    &ObjectSchema::new(
        "Create a new repository snapshot.",
        // schema properties must be sorted alphabetically
        &[
            (
                "dry-run",
                true,
                &BooleanSchema::new("Print what would happen.")
                    .default(false)
                    .schema(),
            ),
            ("id", false, &StringSchema::new("Mirror name.").schema()),
        ],
    ),
);

#[derive(Deserialize)]
#[allow(dead_code)]
struct GlobalArgs {
    config: Option<String>,
}

impl ApiType for GlobalArgs {
    const API_SCHEMA: Schema = ObjectSchema::new(
        "Global args.",
        &[(
            "config",
            true,
            &StringSchema::new("Path to mirroring config file.").schema(),
        )],
    )
    .schema();
}

fn main() {
    let snapshot = CliCommandMap::new().insert(
        "create",
        CliCommand::new(&API_METHOD_CREATE).arg_param(&["id"]),
    );
    let mirror = CliCommandMap::new().insert("snapshot", CommandLineInterface::Nested(snapshot));
    let cmd_def = CliCommandMap::new()
        .global_option(GlobalOptions::of::<GlobalArgs>())
        .insert("mirror", CommandLineInterface::Nested(mirror))
        .build();

    // CommandLine::new auto-inserts a `help` subcommand; no insert_help() call needed.
    CommandLine::new(cmd_def).run(std::env::args(), |_env| Ok(()));
}
