#![allow(clippy::match_bool)] // just no...

use std::collections::{HashMap, HashSet};

use anyhow::{bail, Error};
use serde::Serialize;
use serde_json::Value;

use proxmox_schema::format::{
    get_property_description, get_schema_type_text, DocumentationFormat, ParameterDisplayStyle,
};
use proxmox_schema::*;

use super::{value_to_text, TableFormatOptions};
use super::{CliCommand, CliCommandMap, CommandLineInterface, GlobalOptions};

/// Helper function to format and print result.
///
/// This is implemented for machine generatable formats 'json' and
/// 'json-pretty'. The 'text' format needs to be handled somewhere
/// else.
pub fn format_and_print_result<T: Serialize>(result: &T, output_format: &str) {
    if output_format == "json-pretty" {
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
    } else if output_format == "json" {
        println!("{}", serde_json::to_string(&result).unwrap());
    } else {
        unimplemented!();
    }
}

/// Helper function to format and print result.
///
/// This is implemented for machine generatable formats 'json' and
/// 'json-pretty', and for the 'text' format which generates nicely
/// formatted tables with borders.
pub fn format_and_print_result_full(
    result: &mut Value,
    return_type: &ReturnType,
    output_format: &str,
    options: &TableFormatOptions,
) {
    if return_type.optional && result.is_null() {
        return;
    }

    if output_format == "json-pretty" {
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
    } else if output_format == "json" {
        println!("{}", serde_json::to_string(&result).unwrap());
    } else if output_format == "text" {
        if let Err(err) = value_to_text(std::io::stdout(), result, return_type.schema, options) {
            eprintln!("unable to format result: {}", err);
        }
    } else {
        eprintln!("undefined output format '{}'", output_format);
    }
}

#[deprecated = "to be removed, not meant as a public interface"]
/// Helper to generate command usage text for simple commands.
pub fn generate_usage_str(
    prefix: &str,
    cli_cmd: &CliCommand,
    format: DocumentationFormat,
    indent: &str,
    skip_options: &[&str],
) -> String {
    generate_usage_str_do(
        prefix,
        cli_cmd,
        format,
        indent,
        skip_options,
        [].into_iter(),
    )
}

pub(crate) fn generate_usage_str_do(
    prefix: &str,
    cli_cmd: &CliCommand,
    format: DocumentationFormat,
    indent: &str,
    skip_options: &[&str],
    global_options_iter: impl Iterator<Item = &'static str>,
) -> String {
    let arg_param = cli_cmd.arg_param;
    let fixed_param = &cli_cmd.fixed_param;
    let schema = cli_cmd.info.parameters;

    let mut done_hash = HashSet::<&str>::new();
    for option in skip_options {
        done_hash.insert(option);
    }

    let mut args = String::new();

    for positional_arg in arg_param {
        match schema.lookup(positional_arg) {
            Some((optional, param_schema)) => {
                args.push(' ');

                let is_array = matches!(param_schema, Schema::Array(_));
                if optional {
                    args.push('[');
                }
                if is_array {
                    args.push('{');
                }
                args.push('<');
                args.push_str(positional_arg);
                args.push('>');
                if is_array {
                    args.push('}');
                }
                if optional {
                    args.push(']');
                }

                done_hash.insert(positional_arg);
            }
            None => panic!("no such property '{}' in schema", positional_arg),
        }
    }

    let mut arg_descr = String::new();
    for positional_arg in arg_param {
        let (_optional, param_schema) = schema.lookup(positional_arg).unwrap();
        let param_descr = get_property_description(
            positional_arg,
            param_schema,
            ParameterDisplayStyle::Fixed,
            format,
        );
        if !arg_descr.is_empty() {
            arg_descr.push_str("\n\n");
        }
        arg_descr.push_str(&param_descr);
    }

    let mut options = String::new();

    for (prop, optional, param_schema) in schema.properties() {
        if done_hash.contains(prop) {
            continue;
        }
        if fixed_param.contains_key(prop) {
            continue;
        }

        let type_text = get_schema_type_text(param_schema, ParameterDisplayStyle::Arg);

        let prop_descr =
            get_property_description(prop, param_schema, ParameterDisplayStyle::Arg, format);

        if *optional {
            if !options.is_empty() {
                options.push('\n');
            }
            options.push_str(&prop_descr);
        } else {
            args.push_str(" --");
            args.push_str(prop);
            args.push(' ');
            args.push_str(&type_text);

            if !arg_descr.is_empty() {
                arg_descr.push_str("\n\n");
            }
            arg_descr.push_str(&prop_descr);
        }

        done_hash.insert(prop);
    }

    let option_indicator = if !options.is_empty() {
        " [OPTIONS]"
    } else {
        ""
    };

    let mut text = match format {
        DocumentationFormat::Short => {
            return format!("{}{}{}{}", indent, prefix, args, option_indicator);
        }
        DocumentationFormat::Long => format!("{}{}{}{}", indent, prefix, args, option_indicator),
        DocumentationFormat::Full => format!(
            "{}{}{}{}\n\n{}",
            indent,
            prefix,
            args,
            option_indicator,
            schema.description()
        ),
        DocumentationFormat::ReST => format!(
            "``{}{}{}``\n\n{}",
            prefix,
            args,
            option_indicator,
            schema.description()
        ),
    };

    if !arg_descr.is_empty() {
        text.push_str("\n\n");
        text.push_str(&arg_descr);
    }

    if !options.is_empty() {
        text.push_str("\n\nOptional parameters:\n\n");
        text.push_str(&options);
    }

    let mut global_options = String::new();
    for opt in global_options_iter {
        use std::fmt::Write as _;

        if done_hash.contains(opt) {
            continue;
        }
        if !global_options.is_empty() {
            if matches!(format, DocumentationFormat::ReST) {
                global_options.push_str("\n\n");
            } else {
                global_options.push('\n');
            }
        }
        let _ = match format {
            DocumentationFormat::ReST => write!(global_options, "``--{opt}``"),
            _ => write!(global_options, "--{opt}"),
        };
    }

    if !global_options.is_empty() {
        text.push_str("\n\nInherited group parameters:\n\n");
        text.push_str(&global_options);
    }

    text
}

#[deprecated = "will be removed, not meant to be a public interface"]
/// Print command usage for simple commands to ``stderr``.
pub fn print_simple_usage_error(prefix: &str, cli_cmd: &CliCommand, err_msg: &str) {
    print_simple_usage_error_do(prefix, cli_cmd, err_msg, [].into_iter())
}

/// Print command usage for simple commands to ``stderr``.
pub(crate) fn print_simple_usage_error_do(
    prefix: &str,
    cli_cmd: &CliCommand,
    err_msg: &str,
    global_options_iter: impl Iterator<Item = &'static str>,
) {
    let usage = generate_usage_str_do(
        prefix,
        cli_cmd,
        DocumentationFormat::Long,
        "",
        &[],
        global_options_iter,
    );
    eprint!("Error: {}\nUsage: {}", err_msg, usage);
}

/// Print command usage for nested commands to ``stderr``.
pub fn print_nested_usage_error(prefix: &str, def: &CliCommandMap, err_msg: &str) {
    let usage = generate_nested_usage(prefix, def, DocumentationFormat::Short);
    eprintln!("Error: {}\n\nUsage:\n\n{}", err_msg, usage);
}

/// While going through nested commands, this keeps track of the available global options.
#[derive(Default)]
struct UsageState {
    global_options: Vec<Vec<&'static Schema>>,
}

impl UsageState {
    fn push_global_options(&mut self, options: &HashMap<std::any::TypeId, GlobalOptions>) {
        self.global_options
            .push(options.values().map(|o| o.schema).collect());
    }

    fn pop_global_options(&mut self) {
        self.global_options.pop();
    }

    fn describe_current(&self, prefix: &str, format: DocumentationFormat) -> String {
        use std::fmt::Write as _;

        let Some(opts) = self.global_options.last() else {
            return String::new();
        };

        if opts.is_empty() {
            return String::new();
        }

        if !matches!(
            format,
            DocumentationFormat::ReST | DocumentationFormat::Full
        ) {
            return String::new();
        }

        let mut out = String::new();
        let _ = write!(out, "Options available for command group ``{prefix}``:");
        for opt in opts {
            for (name, _optional, schema) in opt
                .any_object()
                .expect("non-object schema in global optiosn")
                .properties()
            {
                let _ = write!(
                    out,
                    "\n\n{}",
                    get_property_description(name, schema, ParameterDisplayStyle::Arg, format)
                );
            }
        }

        out
    }

    fn global_options_iter(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.global_options
            .iter()
            .flat_map(|list| list.iter().copied())
            .flat_map(|o| o.any_object().unwrap().properties())
            .map(|(name, _optional, _schema)| *name)
    }
}

/// Helper to generate command usage text for nested commands.
pub fn generate_nested_usage(
    prefix: &str,
    def: &CliCommandMap,
    format: DocumentationFormat,
) -> String {
    generate_nested_usage_do(&mut UsageState::default(), prefix, def, format)
}

fn generate_nested_usage_do(
    state: &mut UsageState,
    prefix: &str,
    def: &CliCommandMap,
    format: DocumentationFormat,
) -> String {
    state.push_global_options(&def.global_options);

    let mut cmds: Vec<&String> = def.commands.keys().collect();
    cmds.sort();

    let skip_options = def.usage_skip_options;

    let mut usage = String::new();

    let globals = state.describe_current(prefix, format);
    if !globals.is_empty() {
        if format == DocumentationFormat::ReST {
            usage.push_str("----\n\n");
        }
        usage.push_str(&globals);
    }

    for cmd in cmds {
        if !usage.is_empty() {
            if matches!(
                format,
                DocumentationFormat::ReST | DocumentationFormat::Long
            ) {
                usage.push_str("\n\n");
            } else {
                usage.push('\n');
            }
        }

        let new_prefix = if prefix.is_empty() {
            String::from(cmd)
        } else {
            format!("{} {}", prefix, cmd)
        };

        match def.commands.get(cmd).unwrap() {
            CommandLineInterface::Simple(cli_cmd) => {
                if !usage.is_empty() && format == DocumentationFormat::ReST {
                    usage.push_str("----\n\n");
                }
                usage.push_str(&generate_usage_str_do(
                    &new_prefix,
                    cli_cmd,
                    format,
                    "",
                    skip_options,
                    state.global_options_iter(),
                ));
            }
            CommandLineInterface::Nested(map) => {
                usage.push_str(&generate_nested_usage_do(state, &new_prefix, map, format));
            }
        }
    }

    state.pop_global_options();

    usage
}

/// Print help text to ``stderr``.
pub fn print_help(
    top_def: &CommandLineInterface,
    prefix: String,
    args: &[String],
    verbose: Option<bool>,
) {
    let mut message = String::new();
    match print_help_to(top_def, prefix, args, verbose, &mut message) {
        Ok(()) => print!("{message}"),
        Err(err) => eprintln!("{err}"),
    }
}

pub fn print_help_to(
    top_def: &CommandLineInterface,
    mut prefix: String,
    args: &[String],
    mut verbose: Option<bool>,
    mut to: impl std::fmt::Write,
) -> Result<(), Error> {
    let mut iface = top_def;

    let mut usage_state = UsageState::default();

    for cmd in args {
        if let CommandLineInterface::Nested(map) = iface {
            usage_state.push_global_options(&map.global_options);

            if let Some((full_name, subcmd)) = map.find_command(cmd) {
                iface = subcmd;
                if !prefix.is_empty() {
                    prefix.push(' ');
                }
                prefix.push_str(&full_name);
                continue;
            }
        }
        if prefix.is_empty() {
            bail!("no such command '{}'", cmd);
        } else {
            bail!("no such command '{} {}'", prefix, cmd);
        }
    }

    if verbose.is_none() {
        if let CommandLineInterface::Simple(_) = iface {
            verbose = Some(true);
        }
    }

    let format = match verbose.unwrap_or(false) {
        true => DocumentationFormat::Full,
        false => DocumentationFormat::Short,
    };

    match iface {
        CommandLineInterface::Nested(map) => {
            writeln!(
                to,
                "Usage:\n\n{}",
                generate_nested_usage_do(&mut usage_state, &prefix, map, format)
            )?;
        }
        CommandLineInterface::Simple(cli_cmd) => {
            writeln!(
                to,
                "Usage: {}",
                generate_usage_str_do(
                    &prefix,
                    cli_cmd,
                    format,
                    "",
                    &[],
                    usage_state.global_options_iter()
                )
            )?;
        }
    }

    Ok(())
}
