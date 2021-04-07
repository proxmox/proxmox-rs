//! Data types to decscribe data types.
//!
//! This is loosly based on JSON Schema, but uses static RUST data
//! types. This way we can build completely static API
//! definitions included with the programs read-only text segment.

use std::fmt;

use anyhow::{bail, format_err, Error};
use serde_json::{json, Value};
use url::form_urlencoded;

use crate::api::const_regex::ConstRegexPattern;

/// Error type for schema validation
///
/// The validation functions may produce several error message,
/// i.e. when validation objects, it can produce one message for each
/// erroneous object property.
#[derive(Default, Debug)]
pub struct ParameterError {
    error_list: Vec<Error>,
}

impl std::error::Error for ParameterError {}

// fixme: record parameter names, to make it usefull to display errord
// on HTML forms.
impl ParameterError {
    pub fn new() -> Self {
        Self {
            error_list: Vec::new(),
        }
    }

    pub fn push(&mut self, value: Error) {
        self.error_list.push(value);
    }

    pub fn len(&self) -> usize {
        self.error_list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl fmt::Display for ParameterError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut msg = String::new();

        if !self.is_empty() {
            msg.push_str("parameter verification errors\n\n");
        }

        for item in self.error_list.iter() {
            let s = item.to_string();
            msg.reserve(s.len() + 1);
            msg.push_str(&s);
            msg.push('\n');
        }

        write!(f, "{}", msg)
    }
}

/// Data type to describe boolean values
#[derive(Debug)]
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
pub struct BooleanSchema {
    pub description: &'static str,
    /// Optional default value.
    pub default: Option<bool>,
}

impl BooleanSchema {
    pub const fn new(description: &'static str) -> Self {
        BooleanSchema {
            description,
            default: None,
        }
    }

    pub const fn default(mut self, default: bool) -> Self {
        self.default = Some(default);
        self
    }

    pub const fn schema(self) -> Schema {
        Schema::Boolean(self)
    }
}

/// Data type to describe integer values.
#[derive(Debug)]
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
pub struct IntegerSchema {
    pub description: &'static str,
    /// Optional minimum.
    pub minimum: Option<isize>,
    /// Optional maximum.
    pub maximum: Option<isize>,
    /// Optional default.
    pub default: Option<isize>,
}

impl IntegerSchema {
    pub const fn new(description: &'static str) -> Self {
        IntegerSchema {
            description,
            default: None,
            minimum: None,
            maximum: None,
        }
    }

    pub const fn default(mut self, default: isize) -> Self {
        self.default = Some(default);
        self
    }

    pub const fn minimum(mut self, minimum: isize) -> Self {
        self.minimum = Some(minimum);
        self
    }

    pub const fn maximum(mut self, maximium: isize) -> Self {
        self.maximum = Some(maximium);
        self
    }

    pub const fn schema(self) -> Schema {
        Schema::Integer(self)
    }

    fn check_constraints(&self, value: isize) -> Result<(), Error> {
        if let Some(minimum) = self.minimum {
            if value < minimum {
                bail!(
                    "value must have a minimum value of {} (got {})",
                    minimum,
                    value
                );
            }
        }

        if let Some(maximum) = self.maximum {
            if value > maximum {
                bail!(
                    "value must have a maximum value of {} (got {})",
                    maximum,
                    value
                );
            }
        }

        Ok(())
    }
}

/// Data type to describe (JSON like) number value
#[derive(Debug)]
pub struct NumberSchema {
    pub description: &'static str,
    /// Optional minimum.
    pub minimum: Option<f64>,
    /// Optional maximum.
    pub maximum: Option<f64>,
    /// Optional default.
    pub default: Option<f64>,
}

impl NumberSchema {
    pub const fn new(description: &'static str) -> Self {
        NumberSchema {
            description,
            default: None,
            minimum: None,
            maximum: None,
        }
    }

    pub const fn default(mut self, default: f64) -> Self {
        self.default = Some(default);
        self
    }

    pub const fn minimum(mut self, minimum: f64) -> Self {
        self.minimum = Some(minimum);
        self
    }

    pub const fn maximum(mut self, maximium: f64) -> Self {
        self.maximum = Some(maximium);
        self
    }

    pub const fn schema(self) -> Schema {
        Schema::Number(self)
    }

    fn check_constraints(&self, value: f64) -> Result<(), Error> {
        if let Some(minimum) = self.minimum {
            if value < minimum {
                bail!(
                    "value must have a minimum value of {} (got {})",
                    minimum,
                    value
                );
            }
        }

        if let Some(maximum) = self.maximum {
            if value > maximum {
                bail!(
                    "value must have a maximum value of {} (got {})",
                    maximum,
                    value
                );
            }
        }

        Ok(())
    }
}

#[cfg(feature = "test-harness")]
impl Eq for NumberSchema {}

#[cfg(feature = "test-harness")]
impl PartialEq for NumberSchema {
    fn eq(&self, rhs: &Self) -> bool {
        fn f64_eq(l: Option<f64>, r: Option<f64>) -> bool {
            match (l, r) {
                (None, None) => true,
                (Some(l), Some(r)) => (l - r).abs() < 0.0001,
                _ => false,
            }
        }

        self.description == rhs.description
            && f64_eq(self.minimum, rhs.minimum)
            && f64_eq(self.maximum, rhs.maximum)
            && f64_eq(self.default, rhs.default)
    }
}

/// Data type to describe string values.
#[derive(Debug)]
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
pub struct StringSchema {
    pub description: &'static str,
    /// Optional default value.
    pub default: Option<&'static str>,
    /// Optional minimal length.
    pub min_length: Option<usize>,
    /// Optional maximal length.
    pub max_length: Option<usize>,
    /// Optional microformat.
    pub format: Option<&'static ApiStringFormat>,
    /// A text representation of the format/type (used to generate documentation).
    pub type_text: Option<&'static str>,
}

impl StringSchema {
    pub const fn new(description: &'static str) -> Self {
        StringSchema {
            description,
            default: None,
            min_length: None,
            max_length: None,
            format: None,
            type_text: None,
        }
    }

    pub const fn default(mut self, text: &'static str) -> Self {
        self.default = Some(text);
        self
    }

    pub const fn format(mut self, format: &'static ApiStringFormat) -> Self {
        self.format = Some(format);
        self
    }

    pub const fn type_text(mut self, type_text: &'static str) -> Self {
        self.type_text = Some(type_text);
        self
    }

    pub const fn min_length(mut self, min_length: usize) -> Self {
        self.min_length = Some(min_length);
        self
    }

    pub const fn max_length(mut self, max_length: usize) -> Self {
        self.max_length = Some(max_length);
        self
    }

    pub const fn schema(self) -> Schema {
        Schema::String(self)
    }

    fn check_length(&self, length: usize) -> Result<(), Error> {
        if let Some(min_length) = self.min_length {
            if length < min_length {
                bail!("value must be at least {} characters long", min_length);
            }
        }

        if let Some(max_length) = self.max_length {
            if length > max_length {
                bail!("value may only be {} characters long", max_length);
            }
        }

        Ok(())
    }

    pub fn check_constraints(&self, value: &str) -> Result<(), Error> {
        self.check_length(value.chars().count())?;

        if let Some(ref format) = self.format {
            match format {
                ApiStringFormat::Pattern(regex) => {
                    if !(regex.regex_obj)().is_match(value) {
                        bail!("value does not match the regex pattern");
                    }
                }
                ApiStringFormat::Enum(variants) => {
                    if variants.iter().find(|&e| e.value == value).is_none() {
                        bail!("value '{}' is not defined in the enumeration.", value);
                    }
                }
                ApiStringFormat::PropertyString(subschema) => {
                    parse_property_string(value, subschema)?;
                }
                ApiStringFormat::VerifyFn(verify_fn) => {
                    verify_fn(value)?;
                }
            }
        }

        Ok(())
    }
}

/// Data type to describe array of values.
///
/// All array elements are of the same type, as defined in the `items`
/// schema.
#[derive(Debug)]
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
pub struct ArraySchema {
    pub description: &'static str,
    /// Element type schema.
    pub items: &'static Schema,
    /// Optional minimal length.
    pub min_length: Option<usize>,
    /// Optional maximal length.
    pub max_length: Option<usize>,
}

impl ArraySchema {
    pub const fn new(description: &'static str, item_schema: &'static Schema) -> Self {
        ArraySchema {
            description,
            items: item_schema,
            min_length: None,
            max_length: None,
        }
    }

    pub const fn min_length(mut self, min_length: usize) -> Self {
        self.min_length = Some(min_length);
        self
    }

    pub const fn max_length(mut self, max_length: usize) -> Self {
        self.max_length = Some(max_length);
        self
    }

    pub const fn schema(self) -> Schema {
        Schema::Array(self)
    }

    fn check_length(&self, length: usize) -> Result<(), Error> {
        if let Some(min_length) = self.min_length {
            if length < min_length {
                bail!("array must contain at least {} elements", min_length);
            }
        }

        if let Some(max_length) = self.max_length {
            if length > max_length {
                bail!("array may only contain {} elements", max_length);
            }
        }

        Ok(())
    }
}

/// Property entry in an object schema:
///
/// - `name`: The name of the property
/// - `optional`: Set when the property is optional
/// - `schema`: Property type schema
pub type SchemaPropertyEntry = (&'static str, bool, &'static Schema);

/// Lookup table to Schema properties
///
/// Stores a sorted list of `(name, optional, schema)` tuples:
///
/// - `name`: The name of the property
/// - `optional`: Set when the property is optional
/// - `schema`: Property type schema
///
/// **Note:** The list has to be storted by name, because we use
/// a binary search to find items.
///
/// This is a workaround unless RUST can const_fn `Hash::new()`
pub type SchemaPropertyMap = &'static [SchemaPropertyEntry];

/// Data type to describe objects (maps).
#[derive(Debug)]
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
pub struct ObjectSchema {
    pub description: &'static str,
    /// If set, allow additional properties which are not defined in
    /// the schema.
    pub additional_properties: bool,
    /// Property schema definitions.
    pub properties: SchemaPropertyMap,
    /// Default key name - used by `parse_parameter_string()`
    pub default_key: Option<&'static str>,
}

impl ObjectSchema {
    pub const fn new(description: &'static str, properties: SchemaPropertyMap) -> Self {
        ObjectSchema {
            description,
            properties,
            additional_properties: false,
            default_key: None,
        }
    }

    pub const fn additional_properties(mut self, additional_properties: bool) -> Self {
        self.additional_properties = additional_properties;
        self
    }

    pub const fn default_key(mut self, key: &'static str) -> Self {
        self.default_key = Some(key);
        self
    }

    pub const fn schema(self) -> Schema {
        Schema::Object(self)
    }

    pub fn lookup(&self, key: &str) -> Option<(bool, &Schema)> {
        if let Ok(ind) = self
            .properties
            .binary_search_by_key(&key, |(name, _, _)| name)
        {
            let (_name, optional, prop_schema) = self.properties[ind];
            Some((optional, prop_schema))
        } else {
            None
        }
    }
}

/// Combines multiple *object* schemas into one.
///
/// Note that these are limited to object schemas. Other schemas will produce errors.
///
/// Technically this could also contain an `additional_properties` flag, however, in the JSON
/// Schema, this is not supported, so here we simply assume additional properties to be allowed.
#[derive(Debug)]
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
pub struct AllOfSchema {
    pub description: &'static str,

    /// The parameter is checked against all of the schemas in the list. Note that all schemas must
    /// be object schemas.
    pub list: &'static [&'static Schema],
}

impl AllOfSchema {
    pub const fn new(description: &'static str, list: &'static [&'static Schema]) -> Self {
        Self { description, list }
    }

    pub const fn schema(self) -> Schema {
        Schema::AllOf(self)
    }

    pub fn lookup(&self, key: &str) -> Option<(bool, &Schema)> {
        for entry in self.list {
            match entry {
                Schema::AllOf(s) => {
                    if let Some(v) = s.lookup(key) {
                        return Some(v);
                    }
                }
                Schema::Object(s) => {
                    if let Some(v) = s.lookup(key) {
                        return Some(v);
                    }
                }
                _ => panic!("non-object-schema in `AllOfSchema`"),
            }
        }

        None
    }
}

/// Beside [`ObjectSchema`] we also have an [`AllOfSchema`] which also represents objects.
pub trait ObjectSchemaType {
    fn description(&self) -> &'static str;
    fn lookup(&self, key: &str) -> Option<(bool, &Schema)>;
    fn properties(&self) -> ObjectPropertyIterator;
    fn additional_properties(&self) -> bool;
}

impl ObjectSchemaType for ObjectSchema {
    fn description(&self) -> &'static str {
        self.description
    }

    fn lookup(&self, key: &str) -> Option<(bool, &Schema)> {
        ObjectSchema::lookup(self, key)
    }

    fn properties(&self) -> ObjectPropertyIterator {
        ObjectPropertyIterator {
            schemas: [].iter(),
            properties: Some(self.properties.iter()),
            nested: None,
        }
    }

    fn additional_properties(&self) -> bool {
        self.additional_properties
    }
}

impl ObjectSchemaType for AllOfSchema {
    fn description(&self) -> &'static str {
        self.description
    }

    fn lookup(&self, key: &str) -> Option<(bool, &Schema)> {
        AllOfSchema::lookup(self, key)
    }

    fn properties(&self) -> ObjectPropertyIterator {
        ObjectPropertyIterator {
            schemas: self.list.iter(),
            properties: None,
            nested: None,
        }
    }

    fn additional_properties(&self) -> bool {
        true
    }
}

#[doc(hidden)]
pub struct ObjectPropertyIterator {
    schemas: std::slice::Iter<'static, &'static Schema>,
    properties: Option<std::slice::Iter<'static, SchemaPropertyEntry>>,
    nested: Option<Box<ObjectPropertyIterator>>,
}

impl Iterator for ObjectPropertyIterator {
    type Item = &'static SchemaPropertyEntry;

    fn next(&mut self) -> Option<&'static SchemaPropertyEntry> {
        loop {
            match self.nested.as_mut().and_then(Iterator::next) {
                Some(item) => return Some(item),
                None => self.nested = None,
            }

            match self.properties.as_mut().and_then(Iterator::next) {
                Some(item) => return Some(item),
                None => match self.schemas.next()? {
                    Schema::AllOf(o) => self.nested = Some(Box::new(o.properties())),
                    Schema::Object(o) => self.properties = Some(o.properties.iter()),
                    _ => {
                        self.properties = None;
                        continue;
                    }
                },
            }
        }
    }
}

/// Schemas are used to describe complex data types.
///
/// All schema types implement constant builder methods, and a final
/// `schema()` method to convert them into a `Schema`.
///
/// ```
/// # use proxmox::api::{*, schema::*};
/// #
/// const SIMPLE_OBJECT: Schema = ObjectSchema::new(
///     "A very simple object with 2 properties",
///     &[ // this arrays needs to be storted by name!
///         (
///             "property_one",
///             false /* required */,
///             &IntegerSchema::new("A required integer property.")
///                 .minimum(0)
///                 .maximum(100)
///                 .schema()
///         ),
///         (
///             "property_two",
///             true /* optional */,
///             &BooleanSchema::new("An optional boolean property.")
///                 .default(true)
///                 .schema()
///         ),
///     ],
/// ).schema();
/// ```
#[derive(Debug)]
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
pub enum Schema {
    Null,
    Boolean(BooleanSchema),
    Integer(IntegerSchema),
    Number(NumberSchema),
    String(StringSchema),
    Object(ObjectSchema),
    Array(ArraySchema),
    AllOf(AllOfSchema),
}

/// A string enum entry. An enum entry must have a value and a description.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
pub struct EnumEntry {
    pub value: &'static str,
    pub description: &'static str,
}

impl EnumEntry {
    /// Convenience method as long as we only have 2 mandatory fields in an `EnumEntry`.
    pub const fn new(value: &'static str, description: &'static str) -> Self {
        Self { value, description }
    }
}

/// String microformat definitions.
///
/// Strings are probably the most flexible data type, and there are
/// several ways to define their content.
///
/// ## Enumerations
///
/// Simple list all possible values.
///
/// ```
/// # use proxmox::api::{*, schema::*};
/// const format: ApiStringFormat = ApiStringFormat::Enum(&[
///     EnumEntry::new("vm", "A guest VM run via qemu"),
///     EnumEntry::new("ct", "A guest container run via lxc"),
/// ]);
/// ```
///
/// ## Regular Expressions
///
/// Use a regular expression to describe valid strings.
///
/// ```
/// # use proxmox::api::{*, schema::*};
/// # use proxmox::const_regex;
/// const_regex! {
///     pub SHA256_HEX_REGEX = r"^[a-f0-9]{64}$";
/// }
/// const format: ApiStringFormat = ApiStringFormat::Pattern(&SHA256_HEX_REGEX);
/// ```
///
/// ## Property Strings
///
/// Use a schema to describe complex types encoded as string.
///
/// Arrays are parsed as comma separated lists, i.e: `"1,2,3"`. The
/// list may be sparated by comma, semicolon or whitespace.
///
/// Objects are parsed as comma (or semicolon) separated `key=value` pairs, i.e:
/// `"prop1=2,prop2=test"`. Any whitespace is trimmed from key and value.
///
///
/// **Note:** This only works for types which does not allow using the
/// comma, semicolon or whitespace separator inside the value,
/// i.e. this only works for arrays of simple data types, and objects
/// with simple properties (no nesting).
///
/// ```
/// # use proxmox::api::{*, schema::*};
/// #
/// const PRODUCT_LIST_SCHEMA: Schema =
///             ArraySchema::new("Product List.", &IntegerSchema::new("Product ID").schema())
///                 .min_length(1)
///                 .schema();
///
/// const SCHEMA: Schema = StringSchema::new("A list of Product IDs, comma separated.")
///     .format(&ApiStringFormat::PropertyString(&PRODUCT_LIST_SCHEMA))
///     .schema();
///
/// let res = parse_simple_value("", &SCHEMA);
/// assert!(res.is_err());
///
/// let res = parse_simple_value("1,2,3", &SCHEMA); // parse as String
/// assert!(res.is_ok());
///
/// let data = parse_property_string("1,2", &PRODUCT_LIST_SCHEMA); // parse as Array
/// assert!(data.is_ok());
/// ```
pub enum ApiStringFormat {
    /// Enumerate all valid strings
    Enum(&'static [EnumEntry]),
    /// Use a regular expression to describe valid strings.
    Pattern(&'static ConstRegexPattern),
    /// Use a schema to describe complex types encoded as string.
    PropertyString(&'static Schema),
    /// Use a verification function.
    VerifyFn(fn(&str) -> Result<(), Error>),
}

impl std::fmt::Debug for ApiStringFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ApiStringFormat::VerifyFn(fnptr) => write!(f, "VerifyFn({:p}", fnptr),
            ApiStringFormat::Enum(variants) => write!(f, "Enum({:?}", variants),
            ApiStringFormat::Pattern(regex) => write!(f, "Pattern({:?}", regex),
            ApiStringFormat::PropertyString(schema) => write!(f, "PropertyString({:?}", schema),
        }
    }
}

#[cfg(feature = "test-harness")]
impl Eq for ApiStringFormat {}

#[cfg(feature = "test-harness")]
impl PartialEq for ApiStringFormat {
    fn eq(&self, rhs: &Self) -> bool {
        match (self, rhs) {
            (ApiStringFormat::Enum(l), ApiStringFormat::Enum(r)) => l == r,
            (ApiStringFormat::Pattern(l), ApiStringFormat::Pattern(r)) => l == r,
            (ApiStringFormat::PropertyString(l), ApiStringFormat::PropertyString(r)) => l == r,
            (ApiStringFormat::VerifyFn(l), ApiStringFormat::VerifyFn(r)) => std::ptr::eq(l, r),
            (_, _) => false,
        }
    }
}

/// Parameters are objects, but we have two types of object schemas, the regular one and the
/// `AllOf` schema.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
pub enum ParameterSchema {
    Object(&'static ObjectSchema),
    AllOf(&'static AllOfSchema),
}

impl ObjectSchemaType for ParameterSchema {
    fn description(&self) -> &'static str {
        match self {
            ParameterSchema::Object(o) => o.description(),
            ParameterSchema::AllOf(o) => o.description(),
        }
    }

    fn lookup(&self, key: &str) -> Option<(bool, &Schema)> {
        match self {
            ParameterSchema::Object(o) => o.lookup(key),
            ParameterSchema::AllOf(o) => o.lookup(key),
        }
    }

    fn properties(&self) -> ObjectPropertyIterator {
        match self {
            ParameterSchema::Object(o) => o.properties(),
            ParameterSchema::AllOf(o) => o.properties(),
        }
    }

    fn additional_properties(&self) -> bool {
        match self {
            ParameterSchema::Object(o) => o.additional_properties(),
            ParameterSchema::AllOf(o) => o.additional_properties(),
        }
    }
}

impl From<&'static ObjectSchema> for ParameterSchema {
    fn from(schema: &'static ObjectSchema) -> Self {
        ParameterSchema::Object(schema)
    }
}

impl From<&'static AllOfSchema> for ParameterSchema {
    fn from(schema: &'static AllOfSchema) -> Self {
        ParameterSchema::AllOf(schema)
    }
}

/// Helper function to parse boolean values
///
/// - true:  `1 | on | yes | true`
/// - false: `0 | off | no | false`
pub fn parse_boolean(value_str: &str) -> Result<bool, Error> {
    match value_str.to_lowercase().as_str() {
        "1" | "on" | "yes" | "true" => Ok(true),
        "0" | "off" | "no" | "false" => Ok(false),
        _ => bail!("Unable to parse boolean option."),
    }
}

/// Parse a complex property string (`ApiStringFormat::PropertyString`)
pub fn parse_property_string(value_str: &str, schema: &'static Schema) -> Result<Value, Error> {
    // helper for object/allof schemas:
    fn parse_object<T: Into<ParameterSchema>>(
        value_str: &str,
        schema: T,
        default_key: Option<&'static str>,
    ) -> Result<Value, Error> {
        let mut param_list: Vec<(String, String)> = vec![];
        let key_val_list: Vec<&str> = value_str
            .split(|c: char| c == ',' || c == ';')
            .filter(|s| !s.is_empty())
            .collect();
        for key_val in key_val_list {
            let kv: Vec<&str> = key_val.splitn(2, '=').collect();
            if kv.len() == 2 {
                param_list.push((kv[0].trim().into(), kv[1].trim().into()));
            } else if let Some(key) = default_key {
                param_list.push((key.into(), kv[0].trim().into()));
            } else {
                bail!("Value without key, but schema does not define a default key.");
            }
        }

        parse_parameter_strings(&param_list, schema, true).map_err(Error::from)
    }

    match schema {
        Schema::Object(object_schema) => {
            parse_object(value_str, object_schema, object_schema.default_key)
        }
        Schema::AllOf(all_of_schema) => parse_object(value_str, all_of_schema, None),
        Schema::Array(array_schema) => {
            let mut array: Vec<Value> = vec![];
            let list: Vec<&str> = value_str
                .split(|c: char| c == ',' || c == ';' || char::is_ascii_whitespace(&c))
                .filter(|s| !s.is_empty())
                .collect();

            for value in list {
                match parse_simple_value(value.trim(), &array_schema.items) {
                    Ok(res) => array.push(res),
                    Err(err) => bail!("unable to parse array element: {}", err),
                }
            }
            array_schema.check_length(array.len())?;

            Ok(array.into())
        }
        _ => bail!("Got unexpected schema type."),
    }
}

/// Parse a simple value (no arrays and no objects)
pub fn parse_simple_value(value_str: &str, schema: &Schema) -> Result<Value, Error> {
    let value = match schema {
        Schema::Null => {
            bail!("internal error - found Null schema.");
        }
        Schema::Boolean(_boolean_schema) => {
            let res = parse_boolean(value_str)?;
            Value::Bool(res)
        }
        Schema::Integer(integer_schema) => {
            let res: isize = value_str.parse()?;
            integer_schema.check_constraints(res)?;
            Value::Number(res.into())
        }
        Schema::Number(number_schema) => {
            let res: f64 = value_str.parse()?;
            number_schema.check_constraints(res)?;
            Value::Number(serde_json::Number::from_f64(res).unwrap())
        }
        Schema::String(string_schema) => {
            string_schema.check_constraints(value_str)?;
            Value::String(value_str.into())
        }
        _ => bail!("unable to parse complex (sub) objects."),
    };
    Ok(value)
}

/// Parse key/value pairs and verify with object schema
///
/// - `test_required`: is set, checks if all required properties are
///   present.
pub fn parse_parameter_strings<T: Into<ParameterSchema>>(
    data: &[(String, String)],
    schema: T,
    test_required: bool,
) -> Result<Value, ParameterError> {
    do_parse_parameter_strings(data, schema.into(), test_required)
}

fn do_parse_parameter_strings(
    data: &[(String, String)],
    schema: ParameterSchema,
    test_required: bool,
) -> Result<Value, ParameterError> {
    let mut params = json!({});

    let mut errors = ParameterError::new();

    let additional_properties = schema.additional_properties();

    for (key, value) in data {
        if let Some((_optional, prop_schema)) = schema.lookup(&key) {
            match prop_schema {
                Schema::Array(array_schema) => {
                    if params[key] == Value::Null {
                        params[key] = json!([]);
                    }
                    match params[key] {
                        Value::Array(ref mut array) => {
                            match parse_simple_value(value, &array_schema.items) {
                                Ok(res) => array.push(res), // fixme: check_length??
                                Err(err) => {
                                    errors.push(format_err!("parameter '{}': {}", key, err))
                                }
                            }
                        }
                        _ => errors.push(format_err!(
                            "parameter '{}': expected array - type missmatch",
                            key
                        )),
                    }
                }
                _ => match parse_simple_value(value, prop_schema) {
                    Ok(res) => {
                        if params[key] == Value::Null {
                            params[key] = res;
                        } else {
                            errors.push(format_err!("parameter '{}': duplicate parameter.", key));
                        }
                    }
                    Err(err) => errors.push(format_err!("parameter '{}': {}", key, err)),
                },
            }
        } else if additional_properties {
            match params[key] {
                Value::Null => {
                    params[key] = Value::String(value.to_owned());
                }
                Value::String(ref old) => {
                    params[key] = Value::Array(vec![
                        Value::String(old.to_owned()),
                        Value::String(value.to_owned()),
                    ]);
                }
                Value::Array(ref mut array) => {
                    array.push(Value::String(value.to_string()));
                }
                _ => errors.push(format_err!(
                    "parameter '{}': expected array - type missmatch",
                    key
                )),
            }
        } else {
            errors.push(format_err!(
                "parameter '{}': schema does not allow additional properties.",
                key
            ));
        }
    }

    if test_required && errors.is_empty() {
        for (name, optional, _prop_schema) in schema.properties() {
            if !(*optional) && params[name] == Value::Null {
                errors.push(format_err!(
                    "parameter '{}': parameter is missing and it is not optional.",
                    name
                ));
            }
        }
    }

    if !errors.is_empty() {
        Err(errors)
    } else {
        Ok(params)
    }
}

/// Parse a `form_urlencoded` query string and verify with object schema
/// - `test_required`: is set, checks if all required properties are
///   present.
pub fn parse_query_string<T: Into<ParameterSchema>>(
    query: &str,
    schema: T,
    test_required: bool,
) -> Result<Value, ParameterError> {
    let param_list: Vec<(String, String)> = form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();

    parse_parameter_strings(&param_list, schema.into(), test_required)
}

/// Verify JSON value with `schema`.
pub fn verify_json(data: &Value, schema: &Schema) -> Result<(), Error> {
    match schema {
        Schema::Null => {
            if !data.is_null() {
                bail!("Expected Null, but value is not Null.");
            }
        }
        Schema::Object(object_schema) => verify_json_object(data, object_schema)?,
        Schema::Array(array_schema) => verify_json_array(data, &array_schema)?,
        Schema::Boolean(boolean_schema) => verify_json_boolean(data, &boolean_schema)?,
        Schema::Integer(integer_schema) => verify_json_integer(data, &integer_schema)?,
        Schema::Number(number_schema) => verify_json_number(data, &number_schema)?,
        Schema::String(string_schema) => verify_json_string(data, &string_schema)?,
        Schema::AllOf(all_of_schema) => verify_json_object(data, all_of_schema)?,
    }
    Ok(())
}

/// Verify JSON value using a `StringSchema`.
pub fn verify_json_string(data: &Value, schema: &StringSchema) -> Result<(), Error> {
    if let Some(value) = data.as_str() {
        schema.check_constraints(value)
    } else {
        bail!("Expected string value.");
    }
}

/// Verify JSON value using a `BooleanSchema`.
pub fn verify_json_boolean(data: &Value, _schema: &BooleanSchema) -> Result<(), Error> {
    if !data.is_boolean() {
        bail!("Expected boolean value.");
    }
    Ok(())
}

/// Verify JSON value using an `IntegerSchema`.
pub fn verify_json_integer(data: &Value, schema: &IntegerSchema) -> Result<(), Error> {
    if let Some(value) = data.as_i64() {
        schema.check_constraints(value as isize)
    } else {
        bail!("Expected integer value.");
    }
}

/// Verify JSON value using an `NumberSchema`.
pub fn verify_json_number(data: &Value, schema: &NumberSchema) -> Result<(), Error> {
    if let Some(value) = data.as_f64() {
        schema.check_constraints(value)
    } else {
        bail!("Expected number value.");
    }
}

/// Verify JSON value using an `ArraySchema`.
pub fn verify_json_array(data: &Value, schema: &ArraySchema) -> Result<(), Error> {
    let list = match data {
        Value::Array(ref list) => list,
        Value::Object(_) => bail!("Expected array - got object."),
        _ => bail!("Expected array - got scalar value."),
    };

    schema.check_length(list.len())?;

    for item in list {
        verify_json(item, &schema.items)?;
    }

    Ok(())
}

/// Verify JSON value using an `ObjectSchema`.
pub fn verify_json_object(
    data: &Value,
    schema: &dyn ObjectSchemaType,
) -> Result<(), Error> {
    let map = match data {
        Value::Object(ref map) => map,
        Value::Array(_) => bail!("Expected object - got array."),
        _ => bail!("Expected object - got scalar value."),
    };

    let additional_properties = schema.additional_properties();

    for (key, value) in map {
        if let Some((_optional, prop_schema)) = schema.lookup(&key) {
            let result = match prop_schema {
                Schema::Object(object_schema) => verify_json_object(value, object_schema),
                Schema::Array(array_schema) => verify_json_array(value, array_schema),
                _ => verify_json(value, prop_schema),
            };
            if let Err(err) = result {
                bail!("property '{}': {}", key, err);
            };
        } else if !additional_properties {
            bail!(
                "property '{}': schema does not allow additional properties.",
                key
            );
        }
    }

    for (name, optional, _prop_schema) in schema.properties() {
        if !(*optional) && data[name] == Value::Null {
            bail!(
                "property '{}': property is missing and it is not optional.",
                name
            );
        }
    }

    Ok(())
}

#[test]
fn test_schema1() {
    let schema = Schema::Object(ObjectSchema {
        description: "TEST",
        additional_properties: false,
        properties: &[],
        default_key: None,
    });

    println!("TEST Schema: {:?}", schema);
}

#[test]
fn test_query_string() {
    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[("name", false, &StringSchema::new("Name.").schema())],
        );

        let res = parse_query_string("", &SCHEMA, true);
        assert!(res.is_err());
    }

    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[("name", true, &StringSchema::new("Name.").schema())],
        );

        let res = parse_query_string("", &SCHEMA, true);
        assert!(res.is_ok());
    }

    // TEST min_length and max_length
    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[(
                "name",
                true,
                &StringSchema::new("Name.")
                    .min_length(5)
                    .max_length(10)
                    .schema(),
            )],
        );

        let res = parse_query_string("name=abcd", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("name=abcde", &SCHEMA, true);
        assert!(res.is_ok());

        let res = parse_query_string("name=abcdefghijk", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("name=abcdefghij", &SCHEMA, true);
        assert!(res.is_ok());
    }

    // TEST regex pattern
    crate::const_regex! {
        TEST_REGEX = "test";
        TEST2_REGEX = "^test$";
    }

    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[(
                "name",
                false,
                &StringSchema::new("Name.")
                    .format(&ApiStringFormat::Pattern(&TEST_REGEX))
                    .schema(),
            )],
        );

        let res = parse_query_string("name=abcd", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("name=ateststring", &SCHEMA, true);
        assert!(res.is_ok());
    }

    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[(
                "name",
                false,
                &StringSchema::new("Name.")
                    .format(&ApiStringFormat::Pattern(&TEST2_REGEX))
                    .schema(),
            )],
        );

        let res = parse_query_string("name=ateststring", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("name=test", &SCHEMA, true);
        assert!(res.is_ok());
    }

    // TEST string enums
    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[(
                "name",
                false,
                &StringSchema::new("Name.")
                    .format(&ApiStringFormat::Enum(&[
                        EnumEntry::new("ev1", "desc ev1"),
                        EnumEntry::new("ev2", "desc ev2"),
                    ]))
                    .schema(),
            )],
        );

        let res = parse_query_string("name=noenum", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("name=ev1", &SCHEMA, true);
        assert!(res.is_ok());

        let res = parse_query_string("name=ev2", &SCHEMA, true);
        assert!(res.is_ok());

        let res = parse_query_string("name=ev3", &SCHEMA, true);
        assert!(res.is_err());
    }
}

#[test]
fn test_query_integer() {
    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[("count", false, &IntegerSchema::new("Count.").schema())],
        );

        let res = parse_query_string("", &SCHEMA, true);
        assert!(res.is_err());
    }

    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[(
                "count",
                true,
                &IntegerSchema::new("Count.")
                    .minimum(-3)
                    .maximum(50)
                    .schema(),
            )],
        );

        let res = parse_query_string("", &SCHEMA, true);
        assert!(res.is_ok());

        let res = parse_query_string("count=abc", &SCHEMA, false);
        assert!(res.is_err());

        let res = parse_query_string("count=30", &SCHEMA, false);
        assert!(res.is_ok());

        let res = parse_query_string("count=-1", &SCHEMA, false);
        assert!(res.is_ok());

        let res = parse_query_string("count=300", &SCHEMA, false);
        assert!(res.is_err());

        let res = parse_query_string("count=-30", &SCHEMA, false);
        assert!(res.is_err());

        let res = parse_query_string("count=50", &SCHEMA, false);
        assert!(res.is_ok());

        let res = parse_query_string("count=-3", &SCHEMA, false);
        assert!(res.is_ok());
    }
}

#[test]
fn test_query_boolean() {
    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[("force", false, &BooleanSchema::new("Force.").schema())],
        );

        let res = parse_query_string("", &SCHEMA, true);
        assert!(res.is_err());
    }

    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[("force", true, &BooleanSchema::new("Force.").schema())],
        );

        let res = parse_query_string("", &SCHEMA, true);
        assert!(res.is_ok());

        let res = parse_query_string("a=b", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("force", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("force=yes", &SCHEMA, true);
        assert!(res.is_ok());
        let res = parse_query_string("force=1", &SCHEMA, true);
        assert!(res.is_ok());
        let res = parse_query_string("force=On", &SCHEMA, true);
        assert!(res.is_ok());
        let res = parse_query_string("force=TRUE", &SCHEMA, true);
        assert!(res.is_ok());
        let res = parse_query_string("force=TREU", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("force=NO", &SCHEMA, true);
        assert!(res.is_ok());
        let res = parse_query_string("force=0", &SCHEMA, true);
        assert!(res.is_ok());
        let res = parse_query_string("force=off", &SCHEMA, true);
        assert!(res.is_ok());
        let res = parse_query_string("force=False", &SCHEMA, true);
        assert!(res.is_ok());
    }
}

#[test]
fn test_verify_function() {
    const SCHEMA: ObjectSchema = ObjectSchema::new(
        "Parameters.",
        &[(
            "p1",
            false,
            &StringSchema::new("P1")
                .format(&ApiStringFormat::VerifyFn(|value| {
                    if value == "test" {
                        return Ok(());
                    };
                    bail!("format error");
                }))
                .schema(),
        )],
    );

    let res = parse_query_string("p1=tes", &SCHEMA, true);
    assert!(res.is_err());
    let res = parse_query_string("p1=test", &SCHEMA, true);
    assert!(res.is_ok());
}

#[test]
fn test_verify_complex_object() {
    const NIC_MODELS: ApiStringFormat = ApiStringFormat::Enum(&[
        EnumEntry::new("e1000", "Intel E1000"),
        EnumEntry::new("virtio", "Paravirtualized ethernet device"),
    ]);

    const PARAM_SCHEMA: Schema = ObjectSchema::new(
        "Properties.",
        &[
            (
                "enable",
                true,
                &BooleanSchema::new("Enable device.").schema(),
            ),
            (
                "model",
                false,
                &StringSchema::new("Ethernet device Model.")
                    .format(&NIC_MODELS)
                    .schema(),
            ),
        ],
    )
    .default_key("model")
    .schema();

    const SCHEMA: ObjectSchema = ObjectSchema::new(
        "Parameters.",
        &[(
            "net0",
            false,
            &StringSchema::new("First Network device.")
                .format(&ApiStringFormat::PropertyString(&PARAM_SCHEMA))
                .schema(),
        )],
    );

    let res = parse_query_string("", &SCHEMA, true);
    assert!(res.is_err());

    let res = parse_query_string("test=abc", &SCHEMA, true);
    assert!(res.is_err());

    let res = parse_query_string("net0=model=abc", &SCHEMA, true);
    assert!(res.is_err());

    let res = parse_query_string("net0=model=virtio", &SCHEMA, true);
    assert!(res.is_ok());

    let res = parse_query_string("net0=model=virtio,enable=1", &SCHEMA, true);
    assert!(res.is_ok());

    let res = parse_query_string("net0=virtio,enable=no", &SCHEMA, true);
    assert!(res.is_ok());
}

#[test]
fn test_verify_complex_array() {
    {
        const PARAM_SCHEMA: Schema =
            ArraySchema::new("Integer List.", &IntegerSchema::new("Soemething").schema()).schema();

        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[(
                "list",
                false,
                &StringSchema::new("A list on integers, comma separated.")
                    .format(&ApiStringFormat::PropertyString(&PARAM_SCHEMA))
                    .schema(),
            )],
        );

        let res = parse_query_string("", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("list=", &SCHEMA, true);
        assert!(res.is_ok());

        let res = parse_query_string("list=abc", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("list=1", &SCHEMA, true);
        assert!(res.is_ok());

        let res = parse_query_string("list=2,3,4,5", &SCHEMA, true);
        assert!(res.is_ok());
    }

    {
        const PARAM_SCHEMA: Schema =
            ArraySchema::new("Integer List.", &IntegerSchema::new("Soemething").schema())
                .min_length(1)
                .max_length(3)
                .schema();

        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[(
                "list",
                false,
                &StringSchema::new("A list on integers, comma separated.")
                    .format(&ApiStringFormat::PropertyString(&PARAM_SCHEMA))
                    .schema(),
            )],
        );

        let res = parse_query_string("list=", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("list=1,2,3", &SCHEMA, true);
        assert!(res.is_ok());

        let res = parse_query_string("list=2,3,4,5", &SCHEMA, true);
        assert!(res.is_err());
    }
}

/// API types are "updatable" in order to support derived "Updater" structs more easily.
///
/// By default, any API type is "updatable" by an `Option<Self>`. For types which do not use the
/// `#[api]` macro, this will need to be explicitly created (or derived via `#[derive(Updatable)]`.
pub trait Updatable: Sized {
    type Updater: Updater;
    /// This should always be true for the "default" updaters which are just `Option<T>` types.
    /// Types which are not wrapped in `Option` must set this to `false`.
    const UPDATER_IS_OPTION: bool;

    fn update_from<T>(&mut self, from: Self::Updater, delete: &[T]) -> Result<(), Error>
    where
        T: AsRef<str>;
    fn try_build_from(from: Self::Updater) -> Result<Self, Error>;
}

#[cfg(feature = "api-macro")]
pub use proxmox_api_macro::Updatable;

#[cfg(feature = "api-macro")]
#[doc(hidden)]
pub use proxmox_api_macro::Updater;

macro_rules! basic_updatable {
    ($($ty:ty)*) => {
        $(
            impl Updatable for $ty {
                type Updater = Option<$ty>;
                const UPDATER_IS_OPTION: bool = true;

                fn update_from<T: AsRef<str>>(
                    &mut self,
                    from: Option<$ty>,
                    _delete: &[T],
                ) -> Result<(), Error> {
                    if let Some(val) = from {
                        *self = val;
                    }
                    Ok(())
                }

                fn try_build_from(from: Option<$ty>) -> Result<Self, Error> {
                    from.ok_or_else(|| format_err!("cannot build from None value"))
                }
            }
        )*
    };
}
basic_updatable! { bool u8 u16 u32 u64 i8 i16 i32 i64 usize isize f32 f64 String char }

impl<T> Updatable for Option<T>
where
    T: Updatable,
{
    type Updater = T::Updater;
    const UPDATER_IS_OPTION: bool = true;

    fn update_from<S: AsRef<str>>(&mut self, from: T::Updater, delete: &[S]) -> Result<(), Error> {
        match self {
            Some(val) => val.update_from(from, delete),
            None => {
                *self = Self::try_build_from(from)?;
                Ok(())
            }
        }
    }

    fn try_build_from(from: T::Updater) -> Result<Self, Error> {
        if from.is_empty() {
            Ok(None)
        } else {
            T::try_build_from(from).map(Some)
        }
    }
}

/// A helper type for "Updater" structs. This trait is *not* implemented for an api "base" type
/// when deriving an `Updater` for it, though the generated *updater* type does implement this
/// trait!
///
/// This trait is mostly to figure out if an updater is empty (iow. it should not be applied).
/// In that, it is useful when a type which should have an updater also has optional fields which
/// should not be serialized. Instead of `#[serde(skip_serializing_if = "Option::is_none")]`, this
/// trait's `is_empty` needs to be used via `#[serde(skip_serializing_if = "Updater::is_empty")]`.
pub trait Updater {
    /// Check if the updater is "none" or "empty".
    fn is_empty(&self) -> bool;
}

impl<T> Updater for Vec<T> {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl<T> Updater for Option<T> {
    fn is_empty(&self) -> bool {
        self.is_none()
    }
}
