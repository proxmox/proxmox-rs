use anyhow::{bail, format_err, Error};
use serde_json::Value;
use std::cell::RefCell;
use std::sync::Arc;

use proxmox_schema::format::DocumentationFormat;
use proxmox_schema::*;

use super::environment::CliEnvironment;
use super::getopts;
use super::{
    generate_nested_usage, generate_usage_str_do, print_help, print_nested_usage_error,
    print_simple_usage_error_do, CliCommand, CliCommandMap, CommandLineInterface, GlobalOptions,
};
use crate::{ApiFuture, ApiHandler, ApiMethod, RpcEnvironment};

/// Schema definition for ``--output-format`` parameter.
///
/// - ``text``: command specific text format.
/// - ``json``: JSON, single line.
/// - ``json-pretty``: JSON, human readable.
///
pub const OUTPUT_FORMAT: Schema = StringSchema::new("Output format.")
    .format(&ApiStringFormat::Enum(&[
        EnumEntry::new("text", "plain text output"),
        EnumEntry::new("json", "single-line json formatted output"),
        EnumEntry::new("json-pretty", "pretty-printed json output"),
    ]))
    .schema();

fn parse_arguments<'cli>(
    prefix: &str,
    cli_cmd: &CliCommand,
    args: Vec<String>,
    global_options_iter: impl Iterator<Item = &'cli GlobalOptions>,
) -> Result<Value, Error> {
    let (params, remaining) = match getopts::parse_arguments(
        &args,
        cli_cmd.arg_param,
        &cli_cmd.fixed_param,
        cli_cmd.info.parameters,
    ) {
        Ok((p, r)) => (p, r),
        Err(err) => {
            let err_msg = err.to_string();
            print_simple_usage_error_do(prefix, cli_cmd, &err_msg, global_options_iter);
            return Err(format_err!("{}", err_msg));
        }
    };

    if !remaining.is_empty() {
        let err_msg = format!("got additional arguments: {:?}", remaining);
        print_simple_usage_error_do(prefix, cli_cmd, &err_msg, global_options_iter);
        return Err(format_err!("{}", err_msg));
    }

    Ok(params)
}

async fn handle_simple_command_future(
    prefix: &str,
    cli_cmd: &CliCommand,
    args: Vec<String>,
    mut rpcenv: CliEnvironment,
) -> Result<(), Error> {
    let params = parse_arguments(prefix, cli_cmd, args, [].into_iter())?;

    let result = match cli_cmd.info.handler {
        ApiHandler::Sync(handler) => (handler)(params, cli_cmd.info, &mut rpcenv),
        ApiHandler::StreamingSync(handler) => (handler)(params, cli_cmd.info, &mut rpcenv)
            .and_then(|r| r.to_value().map_err(Error::from)),
        ApiHandler::Async(handler) => (handler)(params, cli_cmd.info, &mut rpcenv).await,
        ApiHandler::StreamingAsync(handler) => (handler)(params, cli_cmd.info, &mut rpcenv)
            .await
            .and_then(|r| r.to_value().map_err(Error::from)),
        #[cfg(feature = "server")]
        ApiHandler::AsyncHttp(_) => {
            bail!("CliHandler does not support ApiHandler::AsyncHttp - internal error")
        }
    };

    match result {
        Ok(value) => {
            if value != Value::Null {
                println!("Result: {}", serde_json::to_string_pretty(&value).unwrap());
            }
        }
        Err(err) => {
            eprintln!("Error: {err:?}");
            return Err(err);
        }
    }

    Ok(())
}

pub(crate) fn handle_simple_command<'cli>(
    prefix: &str,
    cli_cmd: &CliCommand,
    args: Vec<String>,
    rpcenv: &mut CliEnvironment,
    run: Option<fn(ApiFuture) -> Result<Value, Error>>,
    global_options_iter: impl Iterator<Item = &'cli GlobalOptions>,
) -> Result<(), Error> {
    let params = parse_arguments(prefix, cli_cmd, args, global_options_iter)?;

    let result = match cli_cmd.info.handler {
        ApiHandler::Sync(handler) => (handler)(params, cli_cmd.info, rpcenv),
        ApiHandler::StreamingSync(handler) => {
            (handler)(params, cli_cmd.info, rpcenv).and_then(|r| r.to_value().map_err(Error::from))
        }
        ApiHandler::Async(handler) => {
            let run = run.ok_or_else(|| {
                format_err!("CliHandler does not support ApiHandler::Async - internal error")
            })?;
            let future = (handler)(params, cli_cmd.info, rpcenv);
            (run)(future)
        }
        ApiHandler::StreamingAsync(_handler) => {
            bail!("CliHandler does not support ApiHandler::StreamingAsync - internal error");
        }
        #[cfg(feature = "server")]
        ApiHandler::AsyncHttp(_) => {
            bail!("CliHandler does not support ApiHandler::AsyncHttp - internal error");
        }
    };

    match result {
        Ok(value) => {
            if value != Value::Null {
                println!("Result: {}", serde_json::to_string_pretty(&value).unwrap());
            }
        }
        Err(err) => {
            eprintln!("Error: {err:?}");
            return Err(err);
        }
    }

    Ok(())
}

fn parse_nested_command<'a>(
    prefix: &mut String,
    def: &'a CliCommandMap,
    args: &mut Vec<String>,
) -> Result<&'a CliCommand, Error> {
    let mut map = def;

    // Note: Avoid async recursive function, because current rust compiler can't handle that
    loop {
        replace_aliases(args, &map.aliases);

        if args.is_empty() {
            let mut cmds: Vec<&String> = map.commands.keys().collect();
            cmds.sort();

            let list = cmds.iter().fold(String::new(), |mut s, item| {
                if !s.is_empty() {
                    s += ", ";
                }
                s += item;
                s
            });

            let err_msg = format!("no command specified.\nPossible commands: {}", list);
            print_nested_usage_error(prefix, map, &err_msg);
            return Err(format_err!("{}", err_msg));
        }

        let command = args.remove(0);

        let (_, sub_cmd) = match map.find_command(&command) {
            Some(cmd) => cmd,
            None => {
                let err_msg = format!("no such command '{}'", command);
                print_nested_usage_error(prefix, map, &err_msg);
                return Err(format_err!("{}", err_msg));
            }
        };

        *prefix = format!("{} {}", prefix, command);

        match sub_cmd {
            CommandLineInterface::Simple(cli_cmd) => {
                //return handle_simple_command(&prefix, cli_cmd, args).await;
                return Ok(cli_cmd);
            }
            CommandLineInterface::Nested(new_map) => map = new_map,
        }
    }
}

const API_METHOD_COMMAND_HELP: ApiMethod = ApiMethod::new(
    &ApiHandler::Sync(&help_command),
    &ObjectSchema::new(
        "Get help about specified command (or sub-command).",
        &[
            (
                "command",
                true,
                &ArraySchema::new(
                    "Command. This may be a list in order to spefify nested sub-commands.",
                    &StringSchema::new("Name.").schema(),
                )
                .schema(),
            ),
            (
                "verbose",
                true,
                &BooleanSchema::new("Verbose help.").schema(),
            ),
        ],
    ),
);

std::thread_local! {
    static HELP_CONTEXT: RefCell<Option<Arc<CommandLineInterface>>> = const { RefCell::new(None) };
}

fn help_command(
    param: Value,
    _info: &ApiMethod,
    _rpcenv: &mut dyn RpcEnvironment,
) -> Result<Value, Error> {
    let mut command: Vec<String> = param["command"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();

    let verbose = param["verbose"].as_bool();

    HELP_CONTEXT.with(|ctx| match &*ctx.borrow() {
        Some(def) => {
            if let CommandLineInterface::Nested(map) = def.as_ref() {
                replace_aliases(&mut command, &map.aliases);
            }
            print_help(def, String::from(""), &command, verbose);
        }
        None => {
            eprintln!("Sorry, help context not set - internal error.");
        }
    });

    Ok(Value::Null)
}

pub(crate) fn set_help_context(def: Option<Arc<CommandLineInterface>>) {
    HELP_CONTEXT.with(|ctx| {
        *ctx.borrow_mut() = def;
    });
}

pub(crate) fn help_command_def() -> CliCommand {
    CliCommand::new(&API_METHOD_COMMAND_HELP).arg_param(&["command"])
}

pub(crate) fn replace_aliases(
    args: &mut Vec<String>,
    aliases: &[(Vec<&'static str>, Vec<&'static str>)],
) {
    for (old, new) in aliases {
        if args.len() < old.len() {
            continue;
        }
        if old[..] == args[..old.len()] {
            let new_args: Vec<String> = new.iter().map(|s| String::from(*s)).collect();
            let rest = args.split_off(old.len());
            args.truncate(0);
            args.extend(new_args);
            for arg in rest.iter() {
                args.push(arg.clone());
            }
            return;
        }
    }
}

/// Handle command invocation.
///
/// This command gets the command line ``args`` and tries to invoke
/// the corresponding API handler.
pub async fn handle_command_future(
    def: Arc<CommandLineInterface>,
    prefix: &str,
    mut args: Vec<String>,
    rpcenv: CliEnvironment,
) -> Result<(), Error> {
    set_help_context(Some(def.clone()));

    let result = match &*def {
        CommandLineInterface::Simple(ref cli_cmd) => {
            handle_simple_command_future(prefix, cli_cmd, args, rpcenv).await
        }
        CommandLineInterface::Nested(ref map) => {
            let mut prefix = prefix.to_string();
            let cli_cmd = parse_nested_command(&mut prefix, map, &mut args)?;
            handle_simple_command_future(&prefix, cli_cmd, args, rpcenv).await
        }
    };

    set_help_context(None);

    result
}

/// Handle command invocation.
///
/// This command gets the command line ``args`` and tries to invoke
/// the corresponding API handler.
pub fn handle_command(
    def: Arc<CommandLineInterface>,
    prefix: &str,
    mut args: Vec<String>,
    mut rpcenv: CliEnvironment,
    run: Option<fn(ApiFuture) -> Result<Value, Error>>,
) -> Result<(), Error> {
    set_help_context(Some(def.clone()));

    let result = match &*def {
        CommandLineInterface::Simple(ref cli_cmd) => {
            handle_simple_command(prefix, cli_cmd, args, &mut rpcenv, run, [].into_iter())
        }
        CommandLineInterface::Nested(ref map) => {
            let mut prefix = prefix.to_string();
            let cli_cmd = parse_nested_command(&mut prefix, map, &mut args)?;
            handle_simple_command(&prefix, cli_cmd, args, &mut rpcenv, run, [].into_iter())
        }
    };

    set_help_context(None);

    result
}

pub(crate) fn prepare_cli_command<A>(
    def: &CommandLineInterface,
    mut args: A,
) -> (String, Vec<String>)
where
    A: Iterator<Item = String>,
{
    let prefix = args.next().unwrap();
    let prefix = prefix.rsplit('/').next().unwrap().to_string(); // without path

    let args: Vec<String> = args.collect();

    if !args.is_empty() {
        if args[0] == "bashcomplete" {
            def.print_bash_completion();
            std::process::exit(0);
        }

        if args[0] == "printdoc" {
            let usage = match def {
                CommandLineInterface::Simple(cli_cmd) => generate_usage_str_do(
                    &prefix,
                    cli_cmd,
                    DocumentationFormat::ReST,
                    "",
                    &[],
                    [].into_iter(),
                ),
                CommandLineInterface::Nested(map) => {
                    generate_nested_usage(&prefix, map, DocumentationFormat::ReST)
                }
            };
            println!("{}", usage);
            std::process::exit(0);
        }
    }

    (prefix, args)
}

/// Helper to get arguments and invoke the command (async).
///
/// This helper reads arguments with ``std::env::args()``. The first
/// argument is assumed to be the program name, and is passed as ``prefix`` to
/// ``handle_command()``.
///
/// This helper automatically add the help command, and two special
/// sub-command:
///
/// - ``bashcomplete``: Output bash completions instead of running the command.
/// - ``printdoc``: Output ReST documentation.
///
pub async fn run_async_cli_command<C: Into<CommandLineInterface>>(def: C, rpcenv: CliEnvironment) {
    run_async_cli_command_with_args(def, rpcenv, std::env::args()).await
}

/// Helper to get arguments and invoke the command.
///
/// This is the synchronous version of run_async_cli_command. You can
/// pass an optional ``run`` function to execute async commands (else
/// async commands simply fail).
pub fn run_cli_command<C: Into<CommandLineInterface>>(
    def: C,
    rpcenv: CliEnvironment,
    run: Option<fn(ApiFuture) -> Result<Value, Error>>,
) {
    run_cli_command_with_args(def, rpcenv, run, std::env::args())
}

/// Helper to get arguments and invoke the command (async).
///
/// The first argument is assumed to be the program name, and is passed as ``prefix`` to
/// ``handle_command()``.
///
/// This helper automatically add the help command, and two special
/// sub-command:
///
/// - ``bashcomplete``: Output bash completions instead of running the command.
/// - ``printdoc``: Output ReST documentation.
///
pub async fn run_async_cli_command_with_args<A, C>(def: C, rpcenv: CliEnvironment, args: A)
where
    C: Into<CommandLineInterface>,
    A: IntoIterator<Item = String>,
{
    let def = match def.into() {
        CommandLineInterface::Simple(cli_cmd) => CommandLineInterface::Simple(cli_cmd),
        CommandLineInterface::Nested(map) => CommandLineInterface::Nested(map.insert_help()),
    };

    let (prefix, args) = prepare_cli_command(&def, args.into_iter());

    if handle_command_future(Arc::new(def), &prefix, args, rpcenv)
        .await
        .is_err()
    {
        std::process::exit(-1);
    }
}

/// Helper to get arguments and invoke the command.
///
/// This is the synchronous version of run_async_cli_command. You can
/// pass an optional ``run`` function to execute async commands (else
/// async commands simply fail).
pub fn run_cli_command_with_args<A, C>(
    def: C,
    rpcenv: CliEnvironment,
    run: Option<fn(ApiFuture) -> Result<Value, Error>>,
    args: A,
) where
    C: Into<CommandLineInterface>,
    A: IntoIterator<Item = String>,
{
    let def = match def.into() {
        CommandLineInterface::Simple(cli_cmd) => CommandLineInterface::Simple(cli_cmd),
        CommandLineInterface::Nested(map) => CommandLineInterface::Nested(map.insert_help()),
    };

    let (prefix, args) = prepare_cli_command(&def, args.into_iter());

    if handle_command(Arc::new(def), &prefix, args, rpcenv, run).is_err() {
        std::process::exit(-1);
    }
}
