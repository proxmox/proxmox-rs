//! Our 'key: value' config format.

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

use std::collections::HashMap;
use std::io::Write;

use anyhow::{Error, bail, format_err};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use proxmox_schema::{ObjectSchemaType, Schema};

type Object = serde_json::Map<String, Value>;

fn object_schema(schema: &'static Schema) -> Result<&'static dyn ObjectSchemaType, Error> {
    schema
        .any_object()
        .ok_or_else(|| format_err!("invalid schema for config, must be an object schema"))
}

/// Parse a full string representing a config file.
pub fn from_str<T: for<'de> Deserialize<'de>>(
    input: &str,
    schema: &'static Schema,
) -> Result<T, Error> {
    Ok(serde_json::from_value(value_from_str(input, schema)?)?)
}

/// Parse a full string representing a config file.
pub fn value_from_str(input: &str, schema: &'static Schema) -> Result<Value, Error> {
    let schema = object_schema(schema)?;

    let mut config = Object::new();
    let mut lines = input.lines().enumerate().peekable();
    let mut description = String::new();

    while let Some((_, line)) = lines.next_if(|(_, line)| line.starts_with('#')) {
        description.push_str(&line[1..]);
        description.push('\n');
    }

    if !description.is_empty() {
        config.insert("description".to_string(), Value::String(description));
    }

    let mut entries = Vec::new();
    for (lineno, line) in lines {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        if let Some((key, value)) =
            parse_line(line).map_err(|err| format_err!("line {lineno}: {err}"))?
        {
            entries.push((lineno, key, value));
        }
    }

    // oneOf type properties may follow the variant properties whose schema they select.
    let type_data: HashMap<&str, &str> = entries
        .iter()
        .map(|(_lineno, key, value)| (*key, *value))
        .collect();
    for (lineno, key, value) in &entries {
        parse_key_value(&mut config, key, value, schema, &type_data)
            .map_err(|err| format_err!("line {lineno}: {err}"))?;
    }

    Ok(Value::Object(config))
}

/// Parse a single `key: value` line from a config file.
fn parse_line(line: &str) -> Result<Option<(&str, &str)>, Error> {
    if line.starts_with('#') || line.is_empty() {
        return Ok(None);
    }

    let colon = line
        .find(':')
        .ok_or_else(|| format_err!("missing colon to separate key from value"))?;
    if colon == 0 {
        bail!("empty key not allowed");
    }

    let key = &line[..colon];
    let value = line[(colon + 1)..].trim_start();
    Ok(Some((key, value)))
}

/// Lookup the key in the schema, parse the value and insert it into the config object.
fn parse_key_value(
    config: &mut Object,
    key: &str,
    value: &str,
    schema: &'static dyn ObjectSchemaType,
    with_type_field: &dyn proxmox_schema::WithTypeField,
) -> Result<(), Error> {
    let schema = match schema.lookup_schema(key, with_type_field) {
        Some((_optional, schema)) => Some(schema),
        None if schema.additional_properties() => None,
        None => bail!(
            "invalid key '{}' and schema does not allow additional properties",
            key
        ),
    };

    let value = parse_value(value, schema)?;
    config.insert(key.to_owned(), value);
    Ok(())
}

/// For this we can just reuse the schema's "parse_simple_value".
///
/// "Additional" properties (`None` schema) will simply become strings.
///
/// Note that this does not handle Object or Array types at all, so if we want to support them
/// natively without going over a `String` type, we can add this here.
fn parse_value(value: &str, schema: Option<&'static Schema>) -> Result<Value, Error> {
    match schema {
        None => Ok(Value::String(value.to_owned())),
        Some(schema) => schema.parse_simple_value(value),
    }
}

/// Parse a string as a property string into a deserializable type. This is just a short wrapper
/// around deserializing the s
pub fn from_property_string<T>(input: &str, schema: &'static Schema) -> Result<T, Error>
where
    T: for<'de> Deserialize<'de>,
{
    Ok(serde_json::from_value(
        schema.parse_property_string(input)?,
    )?)
}

/// Serialize a data structure using a 'key: value' config file format.
pub fn to_bytes<T: Serialize>(value: &T, schema: &'static Schema) -> Result<Vec<u8>, Error> {
    value_to_bytes(&serde_json::to_value(value)?, schema)
}

/// Serialize a json value using a 'key: value' config file format.
pub fn value_to_bytes(value: &Value, schema: &'static Schema) -> Result<Vec<u8>, Error> {
    let schema = object_schema(schema)?;

    schema.verify_json(value)?;

    let object = value
        .as_object()
        .ok_or_else(|| format_err!("value must be an object"))?;

    let mut out = Vec::new();
    object_to_writer(&mut out, object)?;
    Ok(out)
}

/// Note: the object must have already been verified at this point.
fn object_to_writer(output: &mut dyn Write, object: &Object) -> Result<(), Error> {
    // special key `description` for multi-line notes, must be written before everything else
    if let Some(Value::String(description)) = object.get("description") {
        for lines in description.lines() {
            writeln!(output, "#{lines}")?;
        }
    }

    for (key, value) in object.iter() {
        match value {
            _ if key == "description" => continue, // skip description as we handle it above
            Value::Null => continue,               // delete this entry
            Value::Bool(v) => writeln!(output, "{key}: {v}")?,
            Value::String(v) => {
                if v.as_bytes().contains(&b'\n') {
                    bail!("value for {} contains newlines", key);
                }
                writeln!(output, "{key}: {v}")?
            }
            Value::Number(v) => writeln!(output, "{key}: {v}")?,
            Value::Array(_) => bail!("arrays are not supported in config files"),
            Value::Object(_) => bail!("complex objects are not supported in config files"),
        }
    }
    Ok(())
}

#[test]
fn test_config() {
    use proxmox_schema::{
        AllOfSchema, ApiStringFormat, EnumEntry, IntegerSchema, ObjectSchema, OneOfSchema,
        StringSchema,
    };

    static STRING_SCHEMA: Schema = StringSchema::new("A test string").schema();

    static PROP_COMMON_1: Schema = StringSchema::new("Common 1").schema();
    static PROP_COMMON_2: Schema = IntegerSchema::new("Common 2").schema();
    static PROP_ONE: Schema = StringSchema::new("Property 1").schema();
    static PROP_TWO: Schema = IntegerSchema::new("Property 2").schema();
    static PROP_CLASHING_A: Schema = StringSchema::new("Clashing Property 1").schema();
    static PROP_CLASHING_B: Schema = IntegerSchema::new("Clashing Property 2").schema();

    static PROP_ENUM_TYPE: Schema = StringSchema::new("Type: A or B")
        .format(&ApiStringFormat::Enum(&[
            EnumEntry::new("A", "Type A"),
            EnumEntry::new("B", "Type B"),
        ]))
        .schema();

    static ENUM_COMMON: Schema = ObjectSchema::new(
        "Common Object Data",
        &[
            ("common1", false, &PROP_COMMON_1),
            ("common2", true, &PROP_COMMON_2),
        ],
    )
    .additional_properties(false)
    .schema();

    static ENUM_A: Schema = ObjectSchema::new(
        "Type A Object",
        &[
            ("clashing", false, &PROP_CLASHING_A),
            ("prop-one", false, &PROP_ONE),
        ],
    )
    .additional_properties(false)
    .schema();

    static ENUM_B: Schema = ObjectSchema::new(
        "Type B Object",
        &[
            ("clashing", false, &PROP_CLASHING_B),
            ("prop-two", false, &PROP_TWO),
        ],
    )
    .additional_properties(false)
    .schema();

    static ENUM_ONE_OF: Schema = OneOfSchema::new(
        "Object A or B.",
        &("type", false, &PROP_ENUM_TYPE),
        &[("A", &ENUM_A), ("B", &ENUM_B)],
    )
    .schema();

    static ENUM_SCHEMA: Schema = AllOfSchema::new(
        "An enum with common and clashing values.",
        &[&ENUM_COMMON, &ENUM_ONE_OF],
    )
    .schema();

    static ENUM_STR_SCHEMA: Schema = StringSchema::new("An enum with common and clashing values.")
        .format(&ApiStringFormat::PropertyString(&ENUM_SCHEMA))
        .schema();

    static CONFIG_SCHEMA: Schema = ObjectSchema::new(
        "config schema",
        &[
            ("complex", false, &ENUM_STR_SCHEMA),
            ("prop1", false, &STRING_SCHEMA),
        ],
    )
    .schema();

    const CONFIG_STR: &str = "\
        complex: common1=hey,type=A,prop-one=One,clashing=Text\n\
        prop1: Hello\n\
    ";

    let value = crate::value_from_str(CONFIG_STR, &CONFIG_SCHEMA).expect("failed to parse config");
    let config = crate::value_to_bytes(&value, &CONFIG_SCHEMA).expect("failed to recreate config");
    let config =
        std::str::from_utf8(&config).expect("rewritten config contains non-utf8 characters");
    assert_eq!(
        config, CONFIG_STR,
        "recreated configuration does not match the original"
    );

    // the type selects the schema of a clashing property, even if it comes last
    assert_eq!(
        value_from_str(
            "clashing: 42\ncommon1: hello\nprop-two: 7\ntype: B\n",
            &ENUM_SCHEMA
        )
        .unwrap(),
        serde_json::json!({"clashing": 42, "common1": "hello", "prop-two": 7, "type": "B"}),
    );

    // the first invalid line is reported, repeat as the order would vary between hash maps
    for _ in 0..16 {
        let err = value_from_str("prop-one: One\nclashing: invalid\ntype: B\n", &ENUM_ONE_OF)
            .unwrap_err();
        assert!(err.to_string().contains("'prop-one'"), "{err}");
    }
}
