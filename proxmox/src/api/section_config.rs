//! Configuration file helper class
//!
//! The `SectionConfig` class implements a configuration parser using
//! API Schemas. A Configuration may contain several sections, each
//! identified by an unique ID. Each section has a type, which is
//! associcatied with a Schema.
//!
//! The file syntax is configurable, but defaults to the following syntax:
//!
//! ```ignore
//! <section1_type>: <section1_id>
//!     <key1> <value1>
//!     <key2> <value2>
//!     ...
//!
//! <section2_type>: <section2_id>
//!     <key1> <value1>
//!     ...
//! ```

use anyhow::*;

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;

use serde::de::DeserializeOwned;
use serde::ser::Serialize;
use serde_json::{json, Value};

use super::schema::*;
use crate::try_block;

/// Associates a section type name with a `Schema`.
pub struct SectionConfigPlugin {
    type_name: String,
    properties: &'static ObjectSchema,
    id_property: Option<String>,
}

impl SectionConfigPlugin {
    pub fn new(
        type_name: String,
        id_property: Option<String>,
        properties: &'static ObjectSchema,
    ) -> Self {
        Self {
            type_name,
            properties,
            id_property,
        }
    }

    pub fn get_id_schema(&self) -> Option<&Schema> {
        match &self.id_property {
            Some(id_prop) => {
                if let Some((_, schema)) = self.properties.lookup(&id_prop) {
                    Some(schema)
                } else {
                    None
                }
            }
            None => None,
        }
    }
}

/// Configuration parser and writer using API Schemas
pub struct SectionConfig {
    plugins: HashMap<String, SectionConfigPlugin>,

    id_schema: &'static Schema,
    parse_section_header: fn(&str) -> Option<(String, String)>,
    parse_section_content: fn(&str) -> Option<(String, String)>,
    format_section_header:
        fn(type_name: &str, section_id: &str, data: &Value) -> Result<String, Error>,
    format_section_content:
        fn(type_name: &str, section_id: &str, key: &str, value: &Value) -> Result<String, Error>,
}

enum ParseState<'a> {
    BeforeHeader,
    InsideSection(&'a SectionConfigPlugin, String, Value),
}

/// Interface to manipulate configuration data
#[derive(Debug)]
pub struct SectionConfigData {
    pub sections: HashMap<String, (String, Value)>,
    order: VecDeque<String>,
}

impl Default for SectionConfigData {
    fn default() -> Self {
        Self::new()
    }
}

impl SectionConfigData {
    /// Creates a new instance without any data.
    pub fn new() -> Self {
        Self {
            sections: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    /// Set data for the specified section.
    ///
    /// Please not that this does not verify the data with the
    /// schema. Instead, verification is done inside `write()`.
    pub fn set_data<T: Serialize>(
        &mut self,
        section_id: &str,
        type_name: &str,
        config: T,
    ) -> Result<(), Error> {
        let json = serde_json::to_value(config)?;
        self.sections
            .insert(section_id.to_string(), (type_name.to_string(), json));
        Ok(())
    }

    /// Lookup section data as json `Value`.
    pub fn lookup_json(&self, type_name: &str, id: &str) -> Result<Value, Error> {
        match self.sections.get(id) {
            Some((section_type_name, config)) => {
                if type_name != section_type_name {
                    bail!(
                        "got unexpected type '{}' for {} '{}'",
                        section_type_name,
                        type_name,
                        id
                    );
                }
                Ok(config.clone())
            }
            None => {
                bail!("no such {} '{}'", type_name, id);
            }
        }
    }

    /// Lookup section data as native rust data type.
    pub fn lookup<T: DeserializeOwned>(&self, type_name: &str, id: &str) -> Result<T, Error> {
        let config = self.lookup_json(type_name, id)?;
        let data = T::deserialize(config)?;
        Ok(data)
    }

    /// Record section ordering
    ///
    /// Sections are written in the recorder order.
    pub fn record_order(&mut self, section_id: &str) {
        self.order.push_back(section_id.to_string());
    }

    /// API helper to represent configuration data as array.
    ///
    /// The array representation is useful to display configuration
    /// data with GUI frameworks like ExtJS.
    pub fn convert_to_array(
        &self,
        id_prop: &str,
        digest: Option<&[u8; 32]>,
        skip: &[&str],
    ) -> Value {
        let mut list: Vec<Value> = vec![];

        let digest: Value = match digest {
            Some(v) => crate::tools::digest_to_hex(v).into(),
            None => Value::Null,
        };

        for (section_id, (_, data)) in &self.sections {
            let mut item = data.clone();
            for prop in skip {
                item.as_object_mut().unwrap().remove(*prop);
            }
            item.as_object_mut()
                .unwrap()
                .insert(id_prop.into(), section_id.clone().into());
            if !digest.is_null() {
                item.as_object_mut()
                    .unwrap()
                    .insert("digest".into(), digest.clone());
            }
            list.push(item);
        }

        list.into()
    }

    /// API helper to represent configuration data as typed array.
    ///
    /// The array representation is useful to display configuration
    /// data with GUI frameworks like ExtJS.
    pub fn convert_to_typed_array<T: DeserializeOwned>(
        &self,
        type_name: &str,
    ) -> Result<Vec<T>, Error> {
        let mut list: Vec<T> = vec![];

        for (section_type, data) in self.sections.values() {
            if section_type == type_name {
                list.push(T::deserialize(data.clone())?);
            }
        }

        Ok(list)
    }
}

impl SectionConfig {
    /// Create a new `SectionConfig` using the default syntax.
    ///
    /// To make it usable, you need to register one or more plugins
    /// with `register_plugin()`.
    pub fn new(id_schema: &'static Schema) -> Self {
        Self {
            plugins: HashMap::new(),
            id_schema,
            parse_section_header: Self::default_parse_section_header,
            parse_section_content: Self::default_parse_section_content,
            format_section_header: Self::default_format_section_header,
            format_section_content: Self::default_format_section_content,
        }
    }

    /// Create a new `SectionConfig` using the systemd config file syntax.
    pub fn with_systemd_syntax(id_schema: &'static Schema) -> Self {
        Self {
            plugins: HashMap::new(),
            id_schema,
            parse_section_header: Self::systemd_parse_section_header,
            parse_section_content: Self::systemd_parse_section_content,
            format_section_header: Self::systemd_format_section_header,
            format_section_content: Self::systemd_format_section_content,
        }
    }

    /// Create a new `SectionConfig` using a custom syntax.
    pub fn with_custom_syntax(
        id_schema: &'static Schema,
        parse_section_header: fn(&str) -> Option<(String, String)>,
        parse_section_content: fn(&str) -> Option<(String, String)>,
        format_section_header: fn(
            type_name: &str,
            section_id: &str,
            data: &Value,
        ) -> Result<String, Error>,
        format_section_content: fn(
            type_name: &str,
            section_id: &str,
            key: &str,
            value: &Value,
        ) -> Result<String, Error>,
    ) -> Self {
        Self {
            plugins: HashMap::new(),
            id_schema,
            parse_section_header,
            parse_section_content,
            format_section_header,
            format_section_content,
        }
    }

    /// Register a plugin, which defines the `Schema` for a section type.
    pub fn register_plugin(&mut self, plugin: SectionConfigPlugin) {
        self.plugins.insert(plugin.type_name.clone(), plugin);
    }

    /// Write the configuration data to a String.
    ///
    /// This verifies the whole data using the schemas defined in the
    /// plugins. Please note that `filename` is only used to improve
    /// error messages.
    pub fn write(&self, filename: &str, config: &SectionConfigData) -> Result<String, Error> {
        try_block!({
            let mut list = VecDeque::new();

            let mut done = HashSet::new();

            for section_id in &config.order {
                if config.sections.get(section_id) == None {
                    continue;
                };
                list.push_back(section_id);
                done.insert(section_id);
            }

            for section_id in config.sections.keys() {
                if done.contains(section_id) {
                    continue;
                };
                list.push_back(section_id);
            }

            let mut raw = String::new();

            for section_id in list {
                let (type_name, section_config) = config.sections.get(section_id).unwrap();
                let plugin = self.plugins.get(type_name).unwrap();

                let id_schema = plugin.get_id_schema().unwrap_or(self.id_schema);
                if let Err(err) = parse_simple_value(&section_id, &id_schema) {
                    bail!("syntax error in section identifier: {}", err.to_string());
                }
                if section_id.chars().any(|c| c.is_control()) {
                    bail!("detected unexpected control character in section ID.");
                }
                if let Err(err) = verify_json_object(section_config, plugin.properties) {
                    bail!("verify section '{}' failed - {}", section_id, err);
                }

                if !raw.is_empty() {
                    raw += "\n"
                }

                raw += &(self.format_section_header)(type_name, section_id, section_config)?;

                for (key, value) in section_config.as_object().unwrap() {
                    if let Some(id_property) = &plugin.id_property {
                        if id_property == key {
                            continue; // skip writing out id properties, they are in the section header
                        }
                    }
                    raw += &(self.format_section_content)(type_name, section_id, key, value)?;
                }
            }

            Ok(raw)
        })
        .map_err(|e: Error| format_err!("writing '{}' failed: {}", filename, e))
    }

    /// Parse configuration data.
    ///
    /// This verifies the whole data using the schemas defined in the
    /// plugins. Please note that `filename` is only used to improve
    /// error messages.
    pub fn parse(&self, filename: &str, raw: &str) -> Result<SectionConfigData, Error> {
        let mut state = ParseState::BeforeHeader;

        let test_required_properties = |value: &Value,
                                        schema: &ObjectSchema,
                                        id_property: &Option<String>|
         -> Result<(), Error> {
            for (name, optional, _prop_schema) in schema.properties {
                if let Some(id_property) = id_property {
                    if name == id_property {
                        // the id_property is the section header, skip for requirement check
                        continue;
                    }
                }
                if !*optional && value[name] == Value::Null {
                    return Err(format_err!(
                        "property '{}' is missing and it is not optional.",
                        name
                    ));
                }
            }
            Ok(())
        };

        let mut line_no = 0;

        try_block!({
            let mut result = SectionConfigData::new();

            try_block!({
                for line in raw.lines() {
                    line_no += 1;

                    match state {
                        ParseState::BeforeHeader => {
                            if line.trim().is_empty() {
                                continue;
                            }

                            if let Some((section_type, section_id)) =
                                (self.parse_section_header)(line)
                            {
                                //println!("OKLINE: type: {} ID: {}", section_type, section_id);
                                if let Some(ref plugin) = self.plugins.get(&section_type) {
                                    let id_schema =
                                        plugin.get_id_schema().unwrap_or(self.id_schema);
                                    if let Err(err) = parse_simple_value(&section_id, id_schema) {
                                        bail!(
                                            "syntax error in section identifier: {}",
                                            err.to_string()
                                        );
                                    }
                                    state =
                                        ParseState::InsideSection(plugin, section_id, json!({}));
                                } else {
                                    bail!("unknown section type '{}'", section_type);
                                }
                            } else {
                                bail!("syntax error (expected header)");
                            }
                        }
                        ParseState::InsideSection(plugin, ref mut section_id, ref mut config) => {
                            if line.trim().is_empty() {
                                // finish section
                                test_required_properties(
                                    config,
                                    &plugin.properties,
                                    &plugin.id_property,
                                )?;
                                if let Some(id_property) = &plugin.id_property {
                                    config[id_property] = Value::from(section_id.clone());
                                }
                                result.set_data(section_id, &plugin.type_name, config.take())?;
                                result.record_order(section_id);

                                state = ParseState::BeforeHeader;
                                continue;
                            }
                            if let Some((key, value)) = (self.parse_section_content)(line) {
                                //println!("CONTENT: key: {} value: {}", key, value);

                                let schema = plugin.properties.lookup(&key);
                                let (is_array, prop_schema) = match schema {
                                    Some((_optional, Schema::Array(ArraySchema { items, .. }))) => {
                                        (true, items)
                                    }
                                    Some((_optional, ref prop_schema)) => (false, prop_schema),
                                    None => bail!("unknown property '{}'", key),
                                };

                                let value = match parse_simple_value(&value, prop_schema) {
                                    Ok(value) => value,
                                    Err(err) => {
                                        bail!("property '{}': {}", key, err.to_string());
                                    }
                                };

                                #[allow(clippy::collapsible_if)] // clearer
                                if is_array {
                                    if config[&key] == Value::Null {
                                        config[key] = json!([value]);
                                    } else {
                                        config[key].as_array_mut().unwrap().push(value);
                                    }
                                } else {
                                    if config[&key] == Value::Null {
                                        config[key] = value;
                                    } else {
                                        bail!("duplicate property '{}'", key);
                                    }
                                }
                            } else {
                                bail!("syntax error (expected section properties)");
                            }
                        }
                    }
                }

                if let ParseState::InsideSection(plugin, ref mut section_id, ref mut config) = state
                {
                    // finish section
                    test_required_properties(&config, &plugin.properties, &plugin.id_property)?;
                    if let Some(id_property) = &plugin.id_property {
                        config[id_property] = Value::from(section_id.clone());
                    }
                    result.set_data(&section_id, &plugin.type_name, config)?;
                    result.record_order(&section_id);
                }

                Ok(())
            })
            .map_err(|e| format_err!("line {} - {}", line_no, e))?;

            Ok(result)
        })
        .map_err(|e: Error| format_err!("parsing '{}' failed: {}", filename, e))
    }

    fn default_format_section_header(
        type_name: &str,
        section_id: &str,
        _data: &Value,
    ) -> Result<String, Error> {
        Ok(format!("{}: {}\n", type_name, section_id))
    }

    fn default_format_section_content(
        type_name: &str,
        section_id: &str,
        key: &str,
        value: &Value,
    ) -> Result<String, Error> {
        if let Value::Array(array) = value {
            let mut list = String::new();
            for item in array {
                let line = Self::default_format_section_content(type_name, section_id, key, item)?;
                if !line.is_empty() {
                    list.push_str(&line);
                }
            }
            return Ok(list);
        }

        let text = match value {
            Value::Null => return Ok(String::new()), // return empty string (delete)
            Value::Bool(v) => v.to_string(),
            Value::String(v) => v.to_string(),
            Value::Number(v) => v.to_string(),
            _ => {
                bail!(
                    "got unsupported type in section '{}' key '{}'",
                    section_id,
                    key
                );
            }
        };

        if text.chars().any(|c| c.is_control()) {
            bail!(
                "detected unexpected control character in section '{}' key '{}'",
                section_id,
                key
            );
        }

        Ok(format!("\t{} {}\n", key, text))
    }

    fn default_parse_section_content(line: &str) -> Option<(String, String)> {
        if line.is_empty() {
            return None;
        }

        let first_char = line.chars().next().unwrap();

        if !first_char.is_whitespace() {
            return None;
        }

        let mut kv_iter = line.trim_start().splitn(2, |c: char| c.is_whitespace());

        let key = match kv_iter.next() {
            Some(v) => v.trim(),
            None => return None,
        };

        if key.is_empty() {
            return None;
        }

        let value = match kv_iter.next() {
            Some(v) => v.trim(),
            None => return None,
        };

        Some((key.into(), value.into()))
    }

    fn default_parse_section_header(line: &str) -> Option<(String, String)> {
        if line.is_empty() {
            return None;
        };

        let first_char = line.chars().next().unwrap();

        if !first_char.is_alphabetic() {
            return None;
        }

        let mut head_iter = line.splitn(2, ':');

        let section_type = match head_iter.next() {
            Some(v) => v.trim(),
            None => return None,
        };

        if section_type.is_empty() {
            return None;
        }

        let section_id = match head_iter.next() {
            Some(v) => v.trim(),
            None => return None,
        };

        Some((section_type.into(), section_id.into()))
    }

    fn systemd_format_section_header(
        type_name: &str,
        section_id: &str,
        _data: &Value,
    ) -> Result<String, Error> {
        if type_name != section_id {
            bail!("gut unexpexted section type");
        }

        Ok(format!("[{}]\n", section_id))
    }

    fn systemd_format_section_content(
        type_name: &str,
        section_id: &str,
        key: &str,
        value: &Value,
    ) -> Result<String, Error> {
        if let Value::Array(array) = value {
            let mut list = String::new();
            for item in array {
                let line = Self::systemd_format_section_content(type_name, section_id, key, item)?;
                if !line.is_empty() {
                    list.push_str(&line);
                }
            }
            return Ok(list);
        }

        let text = match value {
            Value::Null => return Ok(String::new()), // return empty string (delete)
            Value::Bool(v) => v.to_string(),
            Value::String(v) => v.to_string(),
            Value::Number(v) => v.to_string(),
            _ => {
                bail!(
                    "got unsupported type in section '{}' key '{}'",
                    section_id,
                    key
                );
            }
        };

        if text.chars().any(|c| c.is_control()) {
            bail!(
                "detected unexpected control character in section '{}' key '{}'",
                section_id,
                key
            );
        }

        Ok(format!("{}={}\n", key, text))
    }

    fn systemd_parse_section_content(line: &str) -> Option<(String, String)> {
        let line = line.trim_end();

        if line.is_empty() {
            return None;
        }

        if line.starts_with(char::is_whitespace) {
            return None;
        }

        let kvpair: Vec<&str> = line.splitn(2, '=').collect();

        if kvpair.len() != 2 {
            return None;
        }

        let key = kvpair[0].trim_end();
        let value = kvpair[1].trim_start();

        Some((key.into(), value.into()))
    }

    fn systemd_parse_section_header(line: &str) -> Option<(String, String)> {
        let line = line.trim_end();

        if line.is_empty() {
            return None;
        };

        if !line.starts_with('[') || !line.ends_with(']') {
            return None;
        }

        let section = line[1..line.len() - 1].trim();

        if section.is_empty() {
            return None;
        }

        Some((section.into(), section.into()))
    }
}

// cargo test test_section_config1 -- --nocapture
#[test]
fn test_section_config1() {
    let filename = "storage.cfg";

    //let mut file = File::open(filename).expect("file not found");
    //let mut contents = String::new();
    //file.read_to_string(&mut contents).unwrap();

    const PROPERTIES: ObjectSchema = ObjectSchema::new(
        "lvmthin properties",
        &[
            (
                "content",
                true,
                &StringSchema::new("Storage content types.").schema(),
            ),
            (
                "thinpool",
                false,
                &StringSchema::new("LVM thin pool name.").schema(),
            ),
            (
                "vgname",
                false,
                &StringSchema::new("LVM volume group name.").schema(),
            ),
        ],
    );

    let plugin = SectionConfigPlugin::new("lvmthin".to_string(), None, &PROPERTIES);

    const ID_SCHEMA: Schema = StringSchema::new("Storage ID schema.")
        .min_length(3)
        .schema();
    let mut config = SectionConfig::new(&ID_SCHEMA);
    config.register_plugin(plugin);

    let raw = r"

lvmthin: local-lvm
        thinpool data
        vgname pve5
        content rootdir,images

lvmthin: local-lvm2
        thinpool data
        vgname pve5
        content rootdir,images
";

    let res = config.parse(filename, &raw);
    println!("RES: {:?}", res);
    let raw = config.write(filename, &res.unwrap());
    println!("CONFIG:\n{}", raw.unwrap());
}

// cargo test test_section_config2 -- --nocapture
#[test]
fn test_section_config2() {
    let filename = "user.cfg";

    const ID_SCHEMA: Schema = StringSchema::new("default id schema.")
        .min_length(3)
        .schema();
    let mut config = SectionConfig::new(&ID_SCHEMA);

    const USER_PROPERTIES: ObjectSchema = ObjectSchema::new(
        "user properties",
        &[
            (
                "email",
                false,
                &StringSchema::new("The e-mail of the user").schema(),
            ),
            (
                "userid",
                true,
                &StringSchema::new("The id of the user (name@realm).")
                    .min_length(3)
                    .schema(),
            ),
        ],
    );

    const GROUP_PROPERTIES: ObjectSchema = ObjectSchema::new(
        "group properties",
        &[
            ("comment", false, &StringSchema::new("Comment").schema()),
            (
                "groupid",
                true,
                &StringSchema::new("The id of the group.")
                    .min_length(3)
                    .schema(),
            ),
        ],
    );

    let plugin = SectionConfigPlugin::new(
        "user".to_string(),
        Some("userid".to_string()),
        &USER_PROPERTIES,
    );
    config.register_plugin(plugin);

    let plugin = SectionConfigPlugin::new(
        "group".to_string(),
        Some("groupid".to_string()),
        &GROUP_PROPERTIES,
    );
    config.register_plugin(plugin);

    let raw = r"

user: root@pam
        email root@example.com

group: mygroup
        comment a very important group
";

    let res = config.parse(filename, &raw);
    println!("RES: {:?}", res);
    let raw = config.write(filename, &res.unwrap());
    println!("CONFIG:\n{}", raw.unwrap());
}
