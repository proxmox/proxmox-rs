//! Module to generate and format API Documentation

use anyhow::{bail, Error};

use crate::*;

/// Enumerate different styles to display parameters/properties.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ParameterDisplayStyle {
    /// Used for properties in configuration files: ``key:``
    Config,
    ///  Used for PropertyStings properties in configuration files
    ConfigSub,
    /// Used for command line options: ``--key``
    Arg,
    /// Used for command line options passed as arguments: ``<key>``
    Fixed,
}

/// CLI usage information format.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum DocumentationFormat {
    /// Text, command line only (one line).
    Short,
    /// Text, list all options.
    Long,
    /// Text, include description.
    Full,
    /// Like full, but in reStructuredText format.
    ReST,
}

/// Line wrapping to form simple list of paragraphs.
pub fn wrap_text(
    initial_indent: &str,
    subsequent_indent: &str,
    text: &str,
    columns: usize,
) -> String {
    // first we condense paragraphs by normalizing whitespace:
    let paragraphs = text
        .split("\n\n")
        .map(|paragraph| {
            paragraph
                .split_ascii_whitespace()
                .fold(String::new(), |mut acc, word| {
                    if !acc.is_empty() {
                        acc.push(' ');
                    }
                    acc.push_str(word);
                    acc
                })
        })
        .collect::<Vec<_>>();

    // Then we wrap each paragraph with textwrap.
    let wrap_options1 = textwrap::Options::new(columns)
        .initial_indent(initial_indent)
        .subsequent_indent(subsequent_indent);

    let wrap_options2 = textwrap::Options::new(columns)
        .initial_indent(subsequent_indent)
        .subsequent_indent(subsequent_indent);

    paragraphs
        .into_iter()
        .filter(|p| !p.is_empty())
        .fold(String::new(), |mut acc, p| {
            if acc.is_empty() {
                acc.push_str(&textwrap::wrap(&p, &wrap_options1).join("\n"));
            } else {
                acc.push_str("\n\n");
                acc.push_str(&textwrap::wrap(&p, &wrap_options2).join("\n"));
            }
            acc
        })
}

#[test]
fn test_wrap_text() {
    let text = "\
Command. This may be a list in order to specify nested subcommands.

A
second
paragraph which will be formatted differently, consisting of both lines
which are too long and ones which are
too
short.";

    let expect = "    \
    Command. This may be a list in order to specify nested
        subcommands.

        A second paragraph which will be formatted
        differently, consisting of both lines which are too
        long and ones which are too short.\
    ";

    let wrapped = wrap_text("    ", "        ", text, 60);

    eprintln!("[[[[\n{expect}]]]]");
    eprintln!("[[[[\n{wrapped}]]]]");

    assert_eq!(wrapped, expect);
}

fn get_simple_type_text(schema: &Schema, list_enums: bool) -> String {
    match schema {
        Schema::Null => String::from("<null>"), // should not happen
        Schema::Boolean(_) => String::from("<1|0>"),
        Schema::Integer(_) => String::from("<integer>"),
        Schema::Number(_) => String::from("<number>"),
        Schema::String(string_schema) => match string_schema {
            StringSchema {
                type_text: Some(type_text),
                ..
            } => String::from(*type_text),
            StringSchema {
                format: Some(ApiStringFormat::Enum(variants)),
                ..
            } => {
                if list_enums && variants.len() <= 3 {
                    let list: Vec<String> =
                        variants.iter().map(|e| String::from(e.value)).collect();
                    list.join("|")
                } else {
                    String::from("<enum>")
                }
            }
            _ => String::from("<string>"),
        },
        _ => panic!("get_simple_type_text: expected simple type"),
    }
}

/// Generate ReST Documentation for object properties
pub fn dump_properties(
    param: &dyn ObjectSchemaType,
    indent: &str,
    style: ParameterDisplayStyle,
    skip: &[&str],
) -> String {
    let mut res = String::new();
    let next_indent = format!("  {indent}");

    let mut required_list: Vec<String> = Vec::new();
    let mut optional_list: Vec<String> = Vec::new();

    for (prop, optional, schema) in param.properties() {
        if skip.iter().any(|n| n == prop) {
            continue;
        }

        let mut param_descr =
            get_property_description(prop, schema, style, DocumentationFormat::ReST);

        if !indent.is_empty() {
            param_descr = format!("{indent}{param_descr}"); // indent first line
            param_descr = param_descr.replace('\n', &format!("\n{indent}")); // indent rest
        }

        if style == ParameterDisplayStyle::Config {
            if let Schema::String(StringSchema {
                format: Some(ApiStringFormat::PropertyString(sub_schema)),
                ..
            }) = schema
            {
                match sub_schema {
                    Schema::Object(object_schema) => {
                        let sub_text = dump_properties(
                            object_schema,
                            &next_indent,
                            ParameterDisplayStyle::ConfigSub,
                            &[],
                        );
                        if !sub_text.is_empty() {
                            param_descr.push_str("\n\n");
                        }
                        param_descr.push_str(&sub_text);
                    }
                    Schema::Array(_) => {
                        // do nothing - description should explain the list type
                    }
                    _ => unreachable!(),
                }
            }
        }
        if *optional {
            optional_list.push(param_descr);
        } else {
            required_list.push(param_descr);
        }
    }

    if !required_list.is_empty() {
        if style != ParameterDisplayStyle::ConfigSub {
            res.push_str("\n*Required properties:*\n\n");
        }

        for text in required_list {
            res.push_str(&text);
            res.push('\n');
        }
    }

    if !optional_list.is_empty() {
        if style != ParameterDisplayStyle::ConfigSub {
            res.push_str("\n*Optional properties:*\n\n");
        }

        for text in optional_list {
            res.push_str(&text);
            res.push('\n');
        }
    }

    res
}

/// Helper to format an object property, including name, type and description.
pub fn get_property_description(
    name: &str,
    schema: &Schema,
    style: ParameterDisplayStyle,
    format: DocumentationFormat,
) -> String {
    let type_text = get_schema_type_text(schema, style);

    let (descr, default, extra) = match schema {
        Schema::Null => ("null", None, None),
        Schema::String(ref schema) => (
            schema.description,
            schema.default.map(|v| v.to_owned()),
            None,
        ),
        Schema::Boolean(ref schema) => (
            schema.description,
            schema.default.map(|v| v.to_string()),
            None,
        ),
        Schema::Integer(ref schema) => (
            schema.description,
            schema.default.map(|v| v.to_string()),
            None,
        ),
        Schema::Number(ref schema) => (
            schema.description,
            schema.default.map(|v| v.to_string()),
            None,
        ),
        Schema::Object(ref schema) => (schema.description, None, None),
        Schema::AllOf(ref schema) => (schema.description, None, None),
        Schema::OneOf(ref schema) => (schema.description, None, None),
        Schema::Array(ref schema) => (
            schema.description,
            None,
            Some(String::from("Can be specified more than once.")),
        ),
    };

    let default_text = match default {
        Some(text) => format!("   (default={text})"),
        None => String::new(),
    };

    let descr = match extra {
        Some(extra) => format!("{descr} {extra}"),
        None => String::from(descr),
    };

    if format == DocumentationFormat::ReST {
        let mut text = match style {
            ParameterDisplayStyle::Config => {
                // reST definition list format
                format!("``{name}`` : ``{type_text}{default_text}``\n")
            }
            ParameterDisplayStyle::ConfigSub => {
                // reST definition list format
                format!("``{name}`` = ``{type_text}{default_text}``\n")
            }
            ParameterDisplayStyle::Arg => {
                // reST option list format
                format!("``--{name}`` ``{type_text}{default_text}``\n")
            }
            ParameterDisplayStyle::Fixed => {
                format!("``<{name}>`` : ``{type_text}{default_text}``\n")
            }
        };

        text.push_str(&wrap_text("  ", "  ", &descr, 80));

        text
    } else {
        let display_name = match style {
            ParameterDisplayStyle::Config => format!("{name}:"),
            ParameterDisplayStyle::ConfigSub => format!("{name}="),
            ParameterDisplayStyle::Arg => format!("--{name}"),
            ParameterDisplayStyle::Fixed => format!("<{name}>"),
        };

        let mut text = format!(" {display_name:-10} {type_text}{default_text}");
        let indent = "             ";
        text.push('\n');
        text.push_str(&wrap_text(indent, indent, &descr, 80));

        text
    }
}

/// Helper to format the type text
///
/// The result is a short string including important constraints, for
/// example ``<integer> (0 - N)``.
pub fn get_schema_type_text(schema: &Schema, _style: ParameterDisplayStyle) -> String {
    match schema {
        Schema::Null => String::from("<null>"), // should not happen
        Schema::String(string_schema) => {
            match string_schema {
                StringSchema {
                    type_text: Some(type_text),
                    ..
                } => String::from(*type_text),
                StringSchema {
                    format: Some(ApiStringFormat::Enum(variants)),
                    ..
                } => {
                    let list: Vec<String> =
                        variants.iter().map(|e| String::from(e.value)).collect();
                    list.join("|")
                }
                // displaying regex add more confision than it helps
                //StringSchema { format: Some(ApiStringFormat::Pattern(const_regex)), .. } => {
                //    format!("/{}/", const_regex.regex_string)
                //}
                StringSchema {
                    format: Some(ApiStringFormat::PropertyString(sub_schema)),
                    ..
                } => get_property_string_type_text(sub_schema),
                _ => String::from("<string>"),
            }
        }
        Schema::Boolean(_) => String::from("<boolean>"),
        Schema::Integer(integer_schema) => match (integer_schema.minimum, integer_schema.maximum) {
            (Some(min), Some(max)) => format!("<integer> ({min} - {max})"),
            (Some(min), None) => format!("<integer> ({min} - N)"),
            (None, Some(max)) => format!("<integer> (-N - {max})"),
            _ => String::from("<integer>"),
        },
        Schema::Number(number_schema) => match (number_schema.minimum, number_schema.maximum) {
            (Some(min), Some(max)) => format!("<number> ({min} - {max})"),
            (Some(min), None) => format!("<number> ({min} - N)"),
            (None, Some(max)) => format!("<number> (-N - {max})"),
            _ => String::from("<number>"),
        },
        Schema::Object(_) => String::from("<object>"),
        Schema::Array(schema) => get_schema_type_text(schema.items, _style),
        Schema::AllOf(_) => String::from("<object>"),
        Schema::OneOf(_) => String::from("<object>"),
    }
}

pub fn get_property_string_type_text(schema: &Schema) -> String {
    match schema {
        Schema::Object(object_schema) => get_object_type_text(object_schema),
        Schema::Array(array_schema) => {
            let item_type = get_simple_type_text(array_schema.items, true);
            format!("[{item_type}, ...]")
        }
        _ => panic!("get_property_string_type_text: expected array or object"),
    }
}

fn get_object_type_text(object_schema: &ObjectSchema) -> String {
    let mut parts = Vec::new();

    let mut add_part = |name, optional, schema| {
        let tt = get_simple_type_text(schema, false);
        let text = if parts.is_empty() {
            format!("{name}={tt}")
        } else {
            format!(",{name}={tt}")
        };
        if optional {
            parts.push(format!("[{text}]"));
        } else {
            parts.push(text);
        }
    };

    // add default key first
    if let Some(ref default_key) = object_schema.default_key {
        let (optional, schema) = object_schema.lookup(default_key).unwrap();
        add_part(default_key, optional, schema);
    }

    // add required keys
    for (name, optional, schema) in object_schema.properties {
        if *optional {
            continue;
        }
        if let Some(ref default_key) = object_schema.default_key {
            if name == default_key {
                continue;
            }
        }
        add_part(name, *optional, schema);
    }

    // add options keys
    for (name, optional, schema) in object_schema.properties {
        if !*optional {
            continue;
        }
        if let Some(ref default_key) = object_schema.default_key {
            if name == default_key {
                continue;
            }
        }
        add_part(name, *optional, schema);
    }

    let mut type_text = String::new();
    type_text.push('[');
    type_text.push_str(&parts.join(" "));
    type_text.push(']');
    type_text
}

/// Generate ReST Documentation for enumeration.
pub fn dump_enum_properties(schema: &Schema) -> Result<String, Error> {
    let mut res = String::new();

    if let Schema::String(StringSchema {
        format: Some(ApiStringFormat::Enum(variants)),
        ..
    }) = schema
    {
        for item in variants.iter() {
            use std::fmt::Write;

            let _ = write!(res, ":``{}``: ", item.value);
            let descr = wrap_text("", "  ", item.description, 80);
            res.push_str(&descr);
            res.push('\n');
        }
        return Ok(res);
    }

    bail!("dump_enum_properties failed - not an enum");
}

pub fn dump_api_return_schema(returns: &ReturnType, style: ParameterDisplayStyle) -> String {
    use std::fmt::Write;

    let schema = &returns.schema;

    let mut res = if returns.optional {
        "*Returns* (optionally): ".to_string()
    } else {
        "*Returns*: ".to_string()
    };

    let type_text = get_schema_type_text(schema, style);
    let _ = write!(res, "**{type_text}**\n\n");

    match schema {
        Schema::Null => {
            return res;
        }
        Schema::Boolean(schema) => {
            let description = wrap_text("", "", schema.description, 80);
            res.push_str(&description);
        }
        Schema::Integer(schema) => {
            let description = wrap_text("", "", schema.description, 80);
            res.push_str(&description);
        }
        Schema::Number(schema) => {
            let description = wrap_text("", "", schema.description, 80);
            res.push_str(&description);
        }
        Schema::String(schema) => {
            let description = wrap_text("", "", schema.description, 80);
            res.push_str(&description);
        }
        Schema::Array(schema) => {
            let description = wrap_text("", "", schema.description, 80);
            res.push_str(&description);
        }
        Schema::Object(obj_schema) => {
            let description = wrap_text("", "", obj_schema.description, 80);
            res.push_str(&description);
            res.push_str(&dump_properties(obj_schema, "", style, &[]));
        }
        Schema::AllOf(all_of_schema) => {
            let description = wrap_text("", "", all_of_schema.description, 80);
            res.push_str(&description);
            res.push_str(&dump_properties(all_of_schema, "", style, &[]));
        }
        Schema::OneOf(all_of_schema) => {
            let description = wrap_text("", "", all_of_schema.description, 80);
            res.push_str(&description);
            res.push_str(&dump_properties(all_of_schema, "", style, &[]));
        }
    }

    res.push('\n');

    res
}
