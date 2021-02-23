#![allow(clippy::match_bool)] // just no...

use serde_json::Value;

use std::collections::HashSet;

use crate::api::format::*;
use crate::api::router::ReturnType;
use crate::api::schema::*;

use super::{value_to_text, TableFormatOptions};
use super::{CliCommand, CliCommandMap, CommandLineInterface};

/// Helper function to format and print result.
///
/// This is implemented for machine generatable formats 'json' and
/// 'json-pretty'. The 'text' format needs to be handled somewhere
/// else.
pub fn format_and_print_result(result: &Value, output_format: &str) {
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
        if let Err(err) = value_to_text(std::io::stdout(), result, &return_type.schema, options) {
            eprintln!("unable to format result: {}", err);
        }
    } else {
        eprintln!("undefined output format '{}'", output_format);
    }
}

/// Helper to generate command usage text for simple commands.
pub fn generate_usage_str(
    prefix: &str,
    cli_cmd: &CliCommand,
    format: DocumentationFormat,
    indent: &str,
    skip_options: &'static [&'static str],
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
            options.push_str(&prop_descr);
        } else {
            args.push_str(" --");
            args.push_str(prop);
            args.push(' ');
            args.push_str(&type_text);

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
            return format!("{}{}{}{}\n", indent, prefix, args, option_indicator);
        }
        DocumentationFormat::Long => format!("{}{}{}{}\n", indent, prefix, args, option_indicator),
        DocumentationFormat::Full => format!(
            "{}{}{}{}\n\n{}\n\n",
            indent,
            prefix,
            args,
            option_indicator,
            schema.description()
        ),
        DocumentationFormat::ReST => format!(
            "``{}{}{}``\n\n{}\n\n",
            prefix,
            args,
            option_indicator,
            schema.description()
        ),
    };

    if !arg_descr.is_empty() {
        text.push_str(&arg_descr);
    }

    if !options.is_empty() {
        text.push_str("Optional parameters:\n\n");
        text.push_str(&options);
    }
    text
}

/// Print command usage for simple commands to ``stderr``.
pub fn print_simple_usage_error(prefix: &str, cli_cmd: &CliCommand, err_msg: &str) {
    let usage = generate_usage_str(prefix, cli_cmd, DocumentationFormat::Long, "", &[]);
    eprint!("Error: {}\nUsage: {}", err_msg, usage);
}

/// Print command usage for nested commands to ``stderr``.
pub fn print_nested_usage_error(prefix: &str, def: &CliCommandMap, err_msg: &str) {
    let usage = generate_nested_usage(prefix, def, DocumentationFormat::Short);
    eprintln!("Error: {}\n\nUsage:\n\n{}", err_msg, usage);
}

/// Helper to generate command usage text for nested commands.
pub fn generate_nested_usage(
    prefix: &str,
    def: &CliCommandMap,
    format: DocumentationFormat,
) -> String {
    let mut cmds: Vec<&String> = def.commands.keys().collect();
    cmds.sort();

    let skip_options = def.usage_skip_options;

    let mut usage = String::new();

    for cmd in cmds {
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
                usage.push_str(&generate_usage_str(
                    &new_prefix,
                    cli_cmd,
                    format,
                    "",
                    skip_options,
                ));
            }
            CommandLineInterface::Nested(map) => {
                usage.push_str(&generate_nested_usage(&new_prefix, map, format));
            }
        }
    }

    usage
}

/// Print help text to ``stderr``.
pub fn print_help(
    top_def: &CommandLineInterface,
    mut prefix: String,
    args: &[String],
    mut verbose: Option<bool>,
) {
    let mut iface = top_def;

    for cmd in args {
        if let CommandLineInterface::Nested(map) = iface {
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
            eprintln!("no such command '{}'", cmd);
        } else {
            eprintln!("no such command '{} {}'", prefix, cmd);
        }
        return;
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
            println!("Usage:\n\n{}", generate_nested_usage(&prefix, map, format));
        }
        CommandLineInterface::Simple(cli_cmd) => {
            println!(
                "Usage: {}",
                generate_usage_str(&prefix, cli_cmd, format, "", &[])
            );
        }
    }
}
