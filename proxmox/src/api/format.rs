//! Module to generate and format API Documenation

use anyhow::{bail, Error};

use std::io::Write;

use crate::api::{
    ApiHandler,
    ApiMethod,
    router::ReturnType,
    section_config::SectionConfig,
    schema::*,
};

/// Enumerate different styles to display parameters/properties.
#[derive(Copy, Clone, PartialEq)]
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
#[derive(Copy, Clone, PartialEq)]
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
    let wrapper1 = textwrap::Wrapper::new(columns)
        .initial_indent(initial_indent)
        .subsequent_indent(subsequent_indent);

    let wrapper2 = textwrap::Wrapper::new(columns)
        .initial_indent(subsequent_indent)
        .subsequent_indent(subsequent_indent);

    text.split("\n\n")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .fold(String::new(), |mut acc, p| {
            if acc.is_empty() {
                acc.push_str(&wrapper1.wrap(p).join("\n"));
            } else {
                acc.push_str(&wrapper2.wrap(p).join("\n"));
            }
            acc.push_str("\n\n");
            acc
        })
}

#[test]
fn test_wrap_text() {
    let text = "Command. This may be a list in order to spefify nested sub-commands.";
    let expect = "             Command. This may be a list in order to spefify nested sub-\n             commands.\n\n";

    let indent = "             ";
    let wrapped = wrap_text(indent, indent, text, 80);

    assert_eq!(wrapped, expect);
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
                StringSchema { type_text: Some(type_text), .. } => {
                    String::from(*type_text)
                }
                StringSchema { format: Some(ApiStringFormat::Enum(variants)), .. } => {
                    let list: Vec<String> = variants.iter().map(|e| String::from(e.value)).collect();
                    list.join("|")
                }
                // displaying regex add more confision than it helps
                //StringSchema { format: Some(ApiStringFormat::Pattern(const_regex)), .. } => {
                //    format!("/{}/", const_regex.regex_string)
                //}
                StringSchema { format: Some(ApiStringFormat::PropertyString(sub_schema)), .. } => {
                    get_property_string_type_text(sub_schema)
                }
                _ => String::from("<string>")
            }
        }
        Schema::Boolean(_) => String::from("<boolean>"),
        Schema::Integer(integer_schema) => match (integer_schema.minimum, integer_schema.maximum) {
            (Some(min), Some(max)) => format!("<integer> ({} - {})", min, max),
            (Some(min), None) => format!("<integer> ({} - N)", min),
            (None, Some(max)) => format!("<integer> (-N - {})", max),
            _ => String::from("<integer>"),
        },
        Schema::Number(number_schema) => match (number_schema.minimum, number_schema.maximum) {
            (Some(min), Some(max)) => format!("<number> ({} - {})", min, max),
            (Some(min), None) => format!("<number> ({} - N)", min),
            (None, Some(max)) => format!("<number> (-N - {})", max),
            _ => String::from("<number>"),
        },
        Schema::Object(_) => String::from("<object>"),
        Schema::Array(_) => String::from("<array>"),
        Schema::AllOf(_) => String::from("<object>"),
    }
}

/// Helper to format an object property, including name, type and description.
pub fn get_property_description(
    name: &str,
    schema: &Schema,
    style: ParameterDisplayStyle,
    format: DocumentationFormat,
) -> String {
    let type_text = get_schema_type_text(schema, style);

    let (descr, default) = match schema {
        Schema::Null => ("null", None),
        Schema::String(ref schema) => (schema.description, schema.default.map(|v| v.to_owned())),
        Schema::Boolean(ref schema) => (schema.description, schema.default.map(|v| v.to_string())),
        Schema::Integer(ref schema) => (schema.description, schema.default.map(|v| v.to_string())),
        Schema::Number(ref schema) => (schema.description, schema.default.map(|v| v.to_string())),
        Schema::Object(ref schema) => (schema.description, None),
        Schema::AllOf(ref schema) => (schema.description, None),
        Schema::Array(ref schema) => (schema.description, None),
    };

    let default_text = match default {
        Some(text) => format!("   (default={})", text),
        None => String::new(),
    };

    if format == DocumentationFormat::ReST {
        let mut text = match style {
            ParameterDisplayStyle::Config => {
                // reST definition list format
                format!("``{}`` : ``{}{}``\n  ", name, type_text, default_text)
            }
            ParameterDisplayStyle::ConfigSub => {
                // reST definition list format
                format!("``{}`` = ``{}{}``\n  ", name, type_text, default_text)
            }
            ParameterDisplayStyle::Arg => {
                // reST option list format
                format!("``--{}`` ``{}{}``\n  ", name, type_text, default_text)
            }
            ParameterDisplayStyle::Fixed => {
                format!("``<{}>`` : ``{}{}``\n  ", name, type_text, default_text)
            }
        };

        text.push_str(&wrap_text("", "  ", descr, 80));
        text.push('\n');

        text
    } else {
        let display_name = match style {
            ParameterDisplayStyle::Config => format!("{}:", name),
            ParameterDisplayStyle::ConfigSub => format!("{}=", name),
            ParameterDisplayStyle::Arg => format!("--{}", name),
            ParameterDisplayStyle::Fixed => format!("<{}>", name),
        };

        let mut text = format!(" {:-10} {}{}", display_name, type_text, default_text);
        let indent = "             ";
        text.push('\n');
        text.push_str(&wrap_text(indent, indent, descr, 80));

        text
    }
}

fn get_simply_type_text(
    schema: &Schema,
    list_enums: bool,
) -> String {

    match schema {
        Schema::Null => String::from("<null>"), // should not happen
        Schema::Boolean(_) => String::from("<1|0>"),
        Schema::Integer(_) => String::from("<integer>"),
        Schema::Number(_) => String::from("<number>"),
        Schema::String(string_schema) => {
            match string_schema {
                StringSchema { type_text: Some(type_text), .. } => {
                    String::from(*type_text)
                }
                StringSchema { format: Some(ApiStringFormat::Enum(variants)), .. } => {
                    if list_enums && variants.len() <= 3 {
                        let list: Vec<String> = variants.iter().map(|e| String::from(e.value)).collect();
                        list.join("|")
                    } else {
                        String::from("<enum>")
                    }
                }
                _ => String::from("<string>"),
            }
        }
        _ => panic!("get_simply_type_text: expected simply type"),
    }
}

fn get_object_type_text(object_schema: &ObjectSchema) -> String {

    let mut parts = Vec::new();

    let mut add_part = |name, optional, schema| {
        let tt = get_simply_type_text(schema, false);
        let text = if parts.is_empty() {
            format!("{}={}", name, tt)
        } else {
            format!(",{}={}", name, tt)
        };
        if optional {
            parts.push(format!("[{}]", text));
        } else {
            parts.push(text);
        }
    };

    // add default key first
    if let Some(ref default_key) = object_schema.default_key {
        let (optional, schema) =  object_schema.lookup(default_key).unwrap();
        add_part(default_key, optional, schema);
    }

    // add required keys
    for (name, optional, schema) in object_schema.properties {
        if *optional { continue; }
        if let Some(ref default_key) = object_schema.default_key {
            if name == default_key { continue; }
        }
        add_part(name, *optional, schema);
    }

    // add options keys
    for (name, optional, schema) in object_schema.properties {
        if !*optional { continue; }
        if let Some(ref default_key) = object_schema.default_key {
            if name == default_key { continue; }
        }
        add_part(name, *optional, schema);
    }

    let mut type_text = String::new();
    type_text.push('[');
    type_text.push_str(&parts.join(" "));
    type_text.push(']');
    type_text
}

fn get_property_string_type_text(
    schema: &Schema,
) -> String {

    match schema {
        Schema::Object(object_schema) => {
            get_object_type_text(object_schema)
        }
        Schema::Array(array_schema) => {
            let item_type = get_simply_type_text(array_schema.items, true);
            format!("[{}, ...]", item_type)
        }
        _ => panic!("get_property_string_type_text: expected array or object"),
    }
}

/// Generate ReST Documentaion for enumeration.
pub fn dump_enum_properties(schema: &Schema) -> Result<String, Error> {

    let mut res = String::new();

    if let Schema::String(StringSchema {
        format: Some(ApiStringFormat::Enum(variants)), ..
    }) = schema {
        for item in variants.iter() {
            res.push_str(&format!(":``{}``: ", item.value));
            let descr = wrap_text("", "  ", item.description, 80);
            res.push_str(&descr);
            res.push('\n');
        }
        return Ok(res);
    }

    bail!("dump_enum_properties failed - not an enum");
}

/// Generate ReST Documentaion for object properties
pub fn dump_properties<I>(
    param: &dyn ObjectSchemaType<PropertyIter = I>,
    indent: &str,
    style: ParameterDisplayStyle,
    skip: &[&str],
) -> String
    where I: Iterator<Item = &'static SchemaPropertyEntry>,
{
    let mut res = String::new();
    let next_indent = format!("  {}", indent);

    let mut required_list: Vec<String> = Vec::new();
    let mut optional_list: Vec<String> = Vec::new();

    for (prop, optional, schema) in param.properties() {

        if skip.iter().find(|n| n == &prop).is_some() { continue; }

        let mut param_descr = get_property_description(
            prop,
            &schema,
            style,
            DocumentationFormat::ReST,
        );

        if !indent.is_empty() {
            param_descr = format!("{}{}", indent, param_descr); // indent first line
            param_descr = param_descr.replace("\n", &format!("\n{}", indent)); // indent rest
        }

        if style == ParameterDisplayStyle::Config {
            match schema {
                Schema::String(StringSchema { format: Some(ApiStringFormat::PropertyString(sub_schema)), .. }) => {
                    match sub_schema {
                        Schema::Object(object_schema) => {
                            let sub_text = dump_properties(
                                object_schema, &next_indent, ParameterDisplayStyle::ConfigSub, &[]);
                            param_descr.push_str(&sub_text);
                        }
                        Schema::Array(_) => {
                            // do nothing - description should explain the list type
                        }
                        _ => unreachable!(),
                    }
                }
                _ => { /* do nothing */ }
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

fn dump_api_return_schema(
    returns: &ReturnType,
    style: ParameterDisplayStyle,
) -> String {
    let schema = &returns.schema;

    let mut res = if returns.optional {
        "*Returns* (optionally): ".to_string()
    } else {
        "*Returns*: ".to_string()
    };

    let type_text = get_schema_type_text(schema, style);
    res.push_str(&format!("**{}**\n\n", type_text));

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
    }

    res.push('\n');

    res
}

fn dump_method_definition(method: &str, path: &str, def: Option<&ApiMethod>) -> Option<String> {
    let style = ParameterDisplayStyle::Config;
    match def {
        None => None,
        Some(api_method) => {

            let description = wrap_text("", "", &api_method.parameters.description(), 80);
            let param_descr = dump_properties(&api_method.parameters, "", style, &[]);

            let return_descr = dump_api_return_schema(&api_method.returns, style);

            let mut method = method;

            if let ApiHandler::AsyncHttp(_) = api_method.handler {
                method = if method == "POST" { "UPLOAD" } else { method };
                method = if method == "GET" { "DOWNLOAD" } else { method };
            }

            let res = format!(
                "**{} {}**\n\n{}{}\n\n{}",
                method, path, description, param_descr, return_descr
            );
            Some(res)
        }
    }
}

/// Generate ReST Documentaion for a complete API defined by a ``Router``.
pub fn dump_api(
    output: &mut dyn Write,
    router: &crate::api::Router,
    path: &str,
    mut pos: usize,
) -> Result<(), Error> {
    use crate::api::SubRoute;

    let mut cond_print = |x| -> Result<_, Error> {
        if let Some(text) = x {
            if pos > 0 {
                writeln!(output, "-----\n")?;
            }
            writeln!(output, "{}", text)?;
            pos += 1;
        }
        Ok(())
    };

    cond_print(dump_method_definition("GET", path, router.get))?;
    cond_print(dump_method_definition("POST", path, router.post))?;
    cond_print(dump_method_definition("PUT", path, router.put))?;
    cond_print(dump_method_definition("DELETE", path, router.delete))?;

    match &router.subroute {
        None => return Ok(()),
        Some(SubRoute::MatchAll { router, param_name }) => {
            let sub_path = if path == "." {
                format!("<{}>", param_name)
            } else {
                format!("{}/<{}>", path, param_name)
            };
            dump_api(output, router, &sub_path, pos)?;
        }
        Some(SubRoute::Map(dirmap)) => {
            //let mut keys: Vec<&String> = map.keys().collect();
            //keys.sort_unstable_by(|a, b| a.cmp(b));
            for (key, sub_router) in dirmap.iter() {
                let sub_path = if path == "." {
                    (*key).to_string()
                } else {
                    format!("{}/{}", path, key)
                };
                dump_api(output, sub_router, &sub_path, pos)?;
            }
        }
    }

    Ok(())
}

/// Generate ReST Documentaion for ``SectionConfig``
pub fn dump_section_config(config: &SectionConfig) -> String {

    let mut res = String::new();

    let plugin_count = config.plugins().len();

    for plugin in config.plugins().values() {

        let name = plugin.type_name();
        let properties = plugin.properties();
        let skip = match plugin.id_property() {
            Some(id) => vec![id],
            None => Vec::new(),
        };

        if plugin_count > 1 {
            let description = wrap_text("", "", &properties.description, 80);
            res.push_str(&format!("\n**Section type** \'``{}``\':  {}\n\n", name, description));
        }

        res.push_str(&dump_properties(properties, "", ParameterDisplayStyle::Config, &skip));
    }

    res
}
