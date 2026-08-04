//! Data types to describe data types.
//!
//! This is loosely based on JSON Schema, but uses static rust data types. This way we can build
//! completely static API definitions that can be included within the programs read-only text
//! segment.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

use anyhow::{Error, bail, format_err};
use serde_json::{Value, json};

use crate::ConstRegexPattern;

/// Error type for schema validation
///
/// The validation functions may produce several error message,
/// i.e. when validation objects, it can produce one message for each
/// erroneous object property.
#[derive(Default, Debug)]
pub struct ParameterError {
    error_list: Vec<(String, Error)>,
}

/// Like anyhow's `format_err` but producing a `ParameterError`.
#[macro_export]
macro_rules! param_format_err {
    ($field:expr, $err:expr) => {
        $crate::ParameterError::from(($field, $err))
    };

    ($field:expr, $($msg:tt)+) => {
        $crate::ParameterError::from(($field, ::anyhow::format_err!($($msg)+)))
    };
}

/// Like anyhow's `bail` but enclosing a `ParameterError`, so
/// a `downcast` can extract it later. This is useful for
/// API calls that need to do parameter checking manually.
#[macro_export]
macro_rules! param_bail {
    ($field:expr, $err:expr) => {{
        return Err($crate::param_format_err!($field, $err).into());
    }};

    ($field:expr, $($msg:tt)+) => {{
        return Err($crate::param_format_err!($field, $($msg)+).into());
    }};
}

impl std::error::Error for ParameterError {}

impl ParameterError {
    pub fn new() -> Self {
        Self {
            error_list: Vec::new(),
        }
    }

    pub fn push(&mut self, name: String, value: Error) {
        self.error_list.push((name, value));
    }

    pub fn len(&self) -> usize {
        self.error_list.len()
    }

    pub fn errors(&self) -> &[(String, Error)] {
        &self.error_list
    }

    pub fn into_inner(self) -> Vec<(String, Error)> {
        self.error_list
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn add_errors(&mut self, prefix: &str, err: Error) {
        match err.downcast::<ParameterError>() {
            Ok(param_err) => {
                self.extend(
                    param_err
                        .into_iter()
                        .map(|(key, err)| (format!("{prefix}/{key}"), err)),
                );
            }
            Err(err) => self.push(prefix.to_string(), err),
        }
    }

    pub(crate) fn from_list(error_list: Vec<(String, Error)>) -> Self {
        Self { error_list }
    }
}

impl fmt::Display for ParameterError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use std::fmt::Write;

        let mut msg = String::new();

        if !self.is_empty() {
            if self.len() == 1 {
                msg.push_str("parameter verification failed - ");
                let _ = write!(msg, "'{}': {}", self.error_list[0].0, self.error_list[0].1);
            } else {
                msg.push_str("parameter verification failed:\n");
                for (name, err) in self.error_list.iter() {
                    let _ = writeln!(msg, "- '{name}': {err}");
                }
            }
        }

        write!(f, "{}", msg.trim())
    }
}

impl From<(String, Error)> for ParameterError {
    fn from(err: (String, Error)) -> Self {
        let mut this = Self::new();
        this.push(err.0, err.1);
        this
    }
}

impl<'a> From<(&'a str, Error)> for ParameterError {
    fn from(err: (&'a str, Error)) -> Self {
        Self::from((err.0.to_string(), err.1))
    }
}

impl std::iter::Extend<(String, Error)> for ParameterError {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = (String, Error)>,
    {
        self.error_list.extend(iter);
    }
}

impl<'a> std::iter::Extend<(&'a str, Error)> for ParameterError {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = (&'a str, Error)>,
    {
        self.extend(iter.into_iter().map(|(s, e)| (s.to_string(), e)));
    }
}

impl IntoIterator for ParameterError {
    type Item = (String, Error);
    type IntoIter = <Vec<(String, Error)> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.into_inner().into_iter()
    }
}

impl FromIterator<(String, Error)> for ParameterError {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = (String, Error)>,
    {
        let mut this = Self::new();
        this.extend(iter);
        this
    }
}

impl<'a> FromIterator<(&'a str, Error)> for ParameterError {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = (&'a str, Error)>,
    {
        let mut this = Self::new();
        this.extend(iter);
        this
    }
}

/// Data type to describe boolean values
#[derive(Debug)]
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
#[non_exhaustive]
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

    pub const fn description(mut self, description: &'static str) -> Self {
        self.description = description;
        self
    }

    pub const fn default(mut self, default: bool) -> Self {
        self.default = Some(default);
        self
    }

    pub const fn schema(self) -> Schema {
        Schema::Boolean(self)
    }

    /// Verify JSON value using a `BooleanSchema`.
    pub fn verify_json(&self, data: &Value) -> Result<(), Error> {
        if !data.is_boolean() {
            bail!("Expected boolean value.");
        }
        Ok(())
    }
}

/// Data type to describe integer values.
#[derive(Debug)]
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
#[non_exhaustive]
pub struct IntegerSchema {
    pub description: &'static str,
    /// Optional minimum.
    pub minimum: Option<i64>,
    /// Optional maximum.
    pub maximum: Option<i64>,
    /// Optional default.
    pub default: Option<i64>,
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

    pub const fn description(mut self, description: &'static str) -> Self {
        self.description = description;
        self
    }

    pub const fn default(mut self, default: i64) -> Self {
        self.default = Some(default);
        self
    }

    pub const fn minimum(mut self, minimum: i64) -> Self {
        self.minimum = Some(minimum);
        self
    }

    pub const fn maximum(mut self, maximum: i64) -> Self {
        self.maximum = Some(maximum);
        self
    }

    pub const fn schema(self) -> Schema {
        Schema::Integer(self)
    }

    pub fn check_constraints(&self, value: i64) -> Result<(), Error> {
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

    /// Verify JSON value using an `IntegerSchema`.
    pub fn verify_json(&self, data: &Value) -> Result<(), Error> {
        if let Some(value) = data.as_i64() {
            self.check_constraints(value)
        } else {
            bail!("Expected integer value.");
        }
    }
}

/// Data type to describe (JSON like) number value
#[derive(Debug)]
#[non_exhaustive]
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

    pub const fn description(mut self, description: &'static str) -> Self {
        self.description = description;
        self
    }

    pub const fn default(mut self, default: f64) -> Self {
        self.default = Some(default);
        self
    }

    pub const fn minimum(mut self, minimum: f64) -> Self {
        self.minimum = Some(minimum);
        self
    }

    pub const fn maximum(mut self, maximum: f64) -> Self {
        self.maximum = Some(maximum);
        self
    }

    pub const fn schema(self) -> Schema {
        Schema::Number(self)
    }

    pub fn check_constraints(&self, value: f64) -> Result<(), Error> {
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

    /// Verify JSON value using an `NumberSchema`.
    pub fn verify_json(&self, data: &Value) -> Result<(), Error> {
        if let Some(value) = data.as_f64() {
            self.check_constraints(value)
        } else {
            bail!("Expected number value.");
        }
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
#[non_exhaustive]
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
    /// Declares the `format` to be a hint for documentation purposes, but other values will be
    /// accepted for backward/forward compatibility purposes.
    pub format_is_optional: bool,
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
            format_is_optional: false,
            type_text: None,
        }
    }

    pub const fn description(mut self, description: &'static str) -> Self {
        self.description = description;
        self
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

    pub const fn format_is_optional(mut self, is_optional: bool) -> Self {
        self.format_is_optional = is_optional;
        self
    }

    pub const fn schema(self) -> Schema {
        Schema::String(self)
    }

    pub(crate) fn check_length(&self, length: usize) -> Result<(), Error> {
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

        if self.format_is_optional {
            return Ok(());
        }

        if let Some(ref format) = self.format {
            match format {
                ApiStringFormat::Pattern(regex) => {
                    if !(regex.regex_obj)().is_match(value) {
                        bail!("value does not match the regex pattern");
                    }
                }
                ApiStringFormat::Enum(variants) => {
                    if !variants.iter().any(|e| e.value == value) {
                        bail!("value '{}' is not defined in the enumeration.", value);
                    }
                }
                ApiStringFormat::PropertyString(subschema) => {
                    crate::de::verify::verify(subschema, value)?;
                }
                ApiStringFormat::VerifyFn(verify_fn) => {
                    verify_fn(value)?;
                }
            }
        }

        Ok(())
    }

    /// Verify JSON value using this `StringSchema`.
    pub fn verify_json(&self, data: &Value) -> Result<(), Error> {
        if let Some(value) = data.as_str() {
            self.check_constraints(value)
        } else {
            bail!("Expected string value.");
        }
    }

    /// Get the [`format`](ApiStringFormat), panics if there is no format.
    pub const fn unwrap_format(&self) -> &'static ApiStringFormat {
        match self.format {
            Some(v) => v,
            None => panic!("unwrap_format on StringSchema without format"),
        }
    }
}

/// Data type to describe array of values.
///
/// All array elements are of the same type, as defined in the `items`
/// schema.
#[derive(Debug)]
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
#[non_exhaustive]
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

    pub const fn description(mut self, description: &'static str) -> Self {
        self.description = description;
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
        Schema::Array(self)
    }

    pub(crate) fn check_length(&self, length: usize) -> Result<(), Error> {
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

    /// Verify JSON value using an `ArraySchema`.
    pub fn verify_json(&self, data: &Value) -> Result<(), Error> {
        let list = match data {
            Value::Array(list) => list,
            Value::Object(_) => bail!("Expected array - got object."),
            _ => bail!("Expected array - got scalar value."),
        };

        self.check_length(list.len())?;

        for (i, item) in list.iter().enumerate() {
            let result = self.items.verify_json(item);
            if let Err(err) = result {
                param_bail!(format!("[{}]", i), err);
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
/// **Note:** The list has to be sorted by name, because we use
/// a binary search to find items.
///
/// This is a workaround unless RUST can const_fn `Hash::new()`
pub type SchemaPropertyMap = &'static [SchemaPropertyEntry];

/// Trait for querying a type field in an object.
///
/// When performing lookups or property iteration, a [`OneOfSchema`] may contain the same field
/// multiple times with different schemas. In order to know which one is to be included callers may
/// pass a [`WithTypeField`] to lookup a type property in the current data, if available.
///
/// Since we usually have a `serde_json::Value` to lookup data in, but don't want to encode that
/// type in the lookup function for flexibility, this trait should be more convenient to use than
/// forcing a closure.
///
/// In order to avoid life time issues and allow more flexible implementations (eg. when the object
/// the data is being fetched from also needs to be mutably accessible in between calls), the code
/// *using* the value is passed as a callback, this way the life time does not need to escapes the
/// `with_type_field` method call itself.
pub trait WithTypeField {
    /// Query the value of a property which is expected to be a string and call a function with it.
    ///
    /// This is called by iterators in order to query the value of [`OneOfSchema`] type properties.
    ///
    /// The value passed to the function is `Some(value)` if the field exists and is a string, or
    /// `None` otherwise.
    fn with_type_field(&self, name: &str, func: &mut dyn FnMut(Option<&str>));
}

impl WithTypeField for Value {
    fn with_type_field(&self, name: &str, func: &mut dyn FnMut(Option<&str>)) {
        func(
            self.as_object()
                .and_then(|obj| obj.get(name))
                .and_then(Value::as_str),
        )
    }
}

impl WithTypeField for serde_json::Map<String, Value> {
    fn with_type_field(&self, name: &str, func: &mut dyn FnMut(Option<&str>)) {
        func(self.get(name).and_then(Value::as_str))
    }
}

/// Covers string to string map types like `HashMap<&str, &str>`.
impl<K, V> WithTypeField for HashMap<K, V>
where
    K: Send + Sync + Eq + std::hash::Hash + std::borrow::Borrow<str>,
    V: Send + Sync + AsRef<str>,
{
    fn with_type_field(&self, name: &str, func: &mut dyn FnMut(Option<&str>)) {
        func(self.get(name).map(|v| v.as_ref()))
    }
}

/// Empty type for when there is no data to use for [`WithTypeField`].
/// That is, when calling [`lookup_schema`](ObjectSchemaType::lookup_schema) on a schema known not
/// to be or contain a `OneOfSchema`, or its contents can safely be ignored.
pub struct NoTypeData;

/// This implementation can be used for `lookup_schema` if no type data is required.
impl WithTypeField for NoTypeData {
    fn with_type_field(&self, _: &str, _: &mut dyn FnMut(Option<&str>)) {}
}

impl<F> WithTypeField for F
where
    F: Fn(&str, &mut dyn FnMut(Option<&str>)),
{
    fn with_type_field(&self, name: &str, func: &mut dyn FnMut(Option<&str>)) {
        (*self)(name, func)
    }
}

const fn assert_properties_sorted(properties: SchemaPropertyMap) {
    use std::cmp::Ordering;

    let mut i = 0;
    let mut prev = None::<&'static str>;
    while i != properties.len() {
        let cur = properties[i].0;
        if let Some(prev) = prev {
            match proxmox_const_utils::byte_string_cmp(prev.as_bytes(), cur.as_bytes()) {
                Ordering::Greater => panic!("object schema properties must be sorted"),
                Ordering::Equal => panic!("duplicate object schema properties not allowed"),
                Ordering::Less => (),
            }
        }
        prev = Some(cur);
        i += 1;
    }
}

const fn property_map_contains(properties: SchemaPropertyMap, needle: &'static str) -> bool {
    use std::cmp::Ordering;

    let mut i = 0;
    while i != properties.len() {
        if let Ordering::Equal =
            proxmox_const_utils::byte_string_cmp(properties[i].0.as_bytes(), needle.as_bytes())
        {
            return true;
        }
        i += 1;
    }
    false
}

const fn assert_key_not_alias(key: &str, aliases: PropertyAliasMap) {
    use std::cmp::Ordering;

    let mut i = 0;
    while i != aliases.len() {
        let (alias, _) = aliases[i];
        if let Ordering::Equal =
            proxmox_const_utils::byte_string_cmp(alias.as_bytes(), key.as_bytes())
        {
            panic!("key must reference a canonical property, not an alias");
        }
        i += 1;
    }
}

const fn assert_property_aliases_valid(aliases: PropertyAliasMap, properties: SchemaPropertyMap) {
    use std::cmp::Ordering;

    let mut i = 0;
    let mut prev = None::<&'static str>;
    while i != aliases.len() {
        let (alias, target) = aliases[i];
        if let Some(prev) = prev {
            match proxmox_const_utils::byte_string_cmp(prev.as_bytes(), alias.as_bytes()) {
                Ordering::Greater => panic!("property aliases must be sorted by alias name"),
                Ordering::Equal => panic!("duplicate property alias not allowed"),
                Ordering::Less => (),
            }
        }
        if property_map_contains(properties, alias) {
            panic!("property alias shadows an existing property of the same name");
        }
        if !property_map_contains(properties, target) {
            panic!("property alias must target an existing property");
        }
        prev = Some(alias);
        i += 1;
    }
}

/// Legacy property strings may contain shortcuts where the *value* of a specific key is used as a
/// *key* for yet another option. Most notably, PVE's `netX` properties use `<model>=<macaddr>`
/// instead of `model=<model>,macaddr=<macaddr>`.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
pub struct KeyAliasInfo {
    pub key_alias: &'static str,
    pub values: &'static [&'static str],
    pub alias: &'static str,
}

impl KeyAliasInfo {
    pub const fn new(
        key_alias: &'static str,
        values: &'static [&'static str],
        alias: &'static str,
    ) -> Self {
        Self {
            key_alias,
            values,
            alias,
        }
    }
}

/// Deprecated-name to canonical-name mapping for object properties.
///
/// Stored sorted by alias name so a binary search can resolve aliases during schema lookup.
pub type PropertyAliasMap = &'static [(&'static str, &'static str)];

/// Data type to describe objects (maps).
#[derive(Debug)]
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
#[non_exhaustive]
pub struct ObjectSchema {
    pub description: &'static str,
    /// If set, allow additional properties which are not defined in
    /// the schema.
    pub additional_properties: bool,
    /// Property schema definitions.
    pub properties: SchemaPropertyMap,
    /// Default key name - used by `parse_parameter_string()`
    pub default_key: Option<&'static str>,
    /// Deprecated property names that resolve to a canonical property in `properties`.
    ///
    /// Each entry is `(alias, canonical)`. Lookups for `alias` behave as if `canonical` had been
    /// used. Parameter parsing rewrites the alias to the canonical name; specifying both at the
    /// same time is rejected with a "cannot set both" error.
    pub property_aliases: PropertyAliasMap,
    /// DO NOT USE!
    ///
    /// This is meant for the PVE schema generator ONLY!
    ///
    /// This is to support legacy property string information: declare a `keyAlias` and its
    /// corresponding `alias` property (as defined in PVE's schema).
    pub key_alias_info: Option<KeyAliasInfo>,
}

impl ObjectSchema {
    /// Create a new `object` schema.
    ///
    /// Note that the `properties` must be sorted! Therefore it is recommended that any schema
    /// created outside of a `const` context is wrapped in a `const {}` block.
    /// (The `#[sortable]` attribute can be used to automate the sorting).
    ///
    /// This is okay:
    ///
    /// ```
    /// # use proxmox_schema::{ObjectSchema, Schema, StringSchema};
    /// const SCHEMA: Schema = ObjectSchema::new(
    ///     "Some Object",
    ///     &[
    ///         ("key1", false, &StringSchema::new("A String").schema()),
    ///         ("key2", false, &StringSchema::new("Another String").schema()),
    ///     ],
    /// ).schema();
    /// ```
    ///
    /// These will panic:
    ///
    /// ```compile_fail,E0080
    /// # use proxmox_schema::{ObjectSchema, Schema, StringSchema};
    /// const SCHEMA: Schema = ObjectSchema::new(
    ///     "Some Object",
    ///     &[
    ///         ("wrong", false, &StringSchema::new("A String").schema()),
    ///         ("order", false, &StringSchema::new("Another String").schema()),
    ///     ],
    /// ).schema();
    /// ```
    ///
    /// ```compile_fail,E0080
    /// # use proxmox_schema::{ObjectSchema, Schema, StringSchema};
    /// const SCHEMA: Schema = ObjectSchema::new(
    ///     "Some Object",
    ///     &[
    ///         ("same", false, &StringSchema::new("A String").schema()),
    ///         // duplicate field name:
    ///         ("same", false, &StringSchema::new("Another String").schema()),
    ///     ],
    /// ).schema();
    /// ```
    pub const fn new(description: &'static str, properties: SchemaPropertyMap) -> Self {
        assert_properties_sorted(properties);
        ObjectSchema {
            description,
            properties,
            additional_properties: false,
            default_key: None,
            property_aliases: &[],
            key_alias_info: None,
        }
    }

    pub const fn additional_properties(mut self, additional_properties: bool) -> Self {
        self.additional_properties = additional_properties;
        self
    }

    pub const fn default_key(mut self, key: &'static str) -> Self {
        assert_key_not_alias(key, self.property_aliases);
        self.default_key = Some(key);
        self
    }

    pub const fn schema(self) -> Schema {
        Schema::Object(self)
    }

    pub fn lookup(&self, key: &str) -> Option<(bool, &'static Schema)> {
        let key = self.lookup_alias(key).unwrap_or(key);
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

    /// Resolve a deprecated property name to its canonical name.
    ///
    /// Returns `None` if `key` is not registered as an alias.
    pub fn lookup_alias(&self, key: &str) -> Option<&'static str> {
        let idx = self
            .property_aliases
            .binary_search_by_key(&key, |(alias, _)| alias)
            .ok()?;
        Some(self.property_aliases[idx].1)
    }

    /// Parse key/value pairs and verify with object schema
    ///
    /// - `test_required`: is set, checks if all required properties are
    ///   present.
    pub fn parse_parameter_strings(
        &'static self,
        data: &[(String, String)],
        test_required: bool,
    ) -> Result<Value, ParameterError> {
        ParameterSchema::from(self).parse_parameter_strings(data, test_required)
    }

    /// Register deprecated property names that resolve to a canonical property.
    ///
    /// Use this to keep an old name accepted after renaming a property: pass
    /// `&[("old-name", "new-name")]` to make CLI flags, property strings, and JSON inputs that
    /// still use `old-name` keep working, while the schema and documentation present only
    /// `new-name`.
    ///
    /// The alias list must be sorted by alias name. Each alias must not shadow an existing
    /// property and must point to an existing property; both are checked at compile time.
    ///
    /// ```
    /// # use proxmox_schema::{ObjectSchema, Schema, StringSchema};
    /// const SCHEMA: Schema = ObjectSchema::new(
    ///     "Some Object",
    ///     &[("mode", false, &StringSchema::new("Mode").schema())],
    /// )
    /// .property_aliases(&[("legacy-mode", "mode")])
    /// .schema();
    /// ```
    ///
    /// Aliases that target a non-existent property are rejected:
    ///
    /// ```compile_fail,E0080
    /// # use proxmox_schema::{ObjectSchema, Schema, StringSchema};
    /// const SCHEMA: Schema = ObjectSchema::new(
    ///     "Some Object",
    ///     &[("mode", false, &StringSchema::new("Mode").schema())],
    /// )
    /// .property_aliases(&[("legacy", "missing")])
    /// .schema();
    /// ```
    ///
    /// Aliases must not shadow an existing property:
    ///
    /// ```compile_fail,E0080
    /// # use proxmox_schema::{ObjectSchema, Schema, StringSchema};
    /// const SCHEMA: Schema = ObjectSchema::new(
    ///     "Some Object",
    ///     &[
    ///         ("mode", false, &StringSchema::new("Mode").schema()),
    ///         ("name", false, &StringSchema::new("Name").schema()),
    ///     ],
    /// )
    /// .property_aliases(&[("name", "mode")])
    /// .schema();
    /// ```
    pub const fn property_aliases(mut self, aliases: PropertyAliasMap) -> Self {
        assert_property_aliases_valid(aliases, self.properties);
        if let Some(default_key) = self.default_key {
            assert_key_not_alias(default_key, aliases);
        }
        self.property_aliases = aliases;
        self
    }

    /// DO NOT USE!
    ///
    /// This is meant for the PVE schema generator ONLY!
    ///
    /// This is to support legacy property string information: declare a `keyAlias` and its
    /// corresponding `alias` property (as defined in PVE's schema).
    pub const fn key_alias_info(mut self, key_alias_info: KeyAliasInfo) -> Self {
        self.key_alias_info = Some(key_alias_info);
        self
    }

    /// Check whether the schema contains a specific parameter.
    /// This is meant for compile time checks and not as public API.
    #[doc(hidden)]
    pub const fn const_has_parameter(&self, name: &[u8], additional_properties: bool) -> bool {
        // additional properties are not statically known, so any name could exist
        if additional_properties && self.additional_properties {
            return true;
        }
        let mut i = 0;
        while i < self.properties.len() {
            if proxmox_const_utils::byte_string_eq(self.properties[i].0.as_bytes(), name) {
                return true;
            }
            i += 1;
        }
        false
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
#[non_exhaustive]
pub struct AllOfSchema {
    pub description: &'static str,

    /// The parameter is checked against all of the schemas in the list. Note that all schemas must
    /// be object schemas.
    pub list: &'static [&'static Schema],
}

const fn assert_only_objects_in_all_of(list: &'static [&'static Schema]) {
    let mut i = 0;
    while i != list.len() {
        match list[i] {
            Schema::Object(_) => (),
            Schema::AllOf(sub) => assert_only_objects_in_all_of(sub.list),
            Schema::OneOf(sub) => assert_only_objects_in_one_of(sub.list),
            _ => panic!("non-object schema in all-of schema"),
        }
        i += 1;
    }
}

impl AllOfSchema {
    pub const fn new(description: &'static str, list: &'static [&'static Schema]) -> Self {
        assert_only_objects_in_all_of(list);
        Self { description, list }
    }

    pub const fn schema(self) -> Schema {
        Schema::AllOf(self)
    }

    pub fn lookup(&self, key: &str) -> Option<(bool, &'static Schema)> {
        for entry in self.list {
            if let Some(v) = entry
                .any_object()
                .expect("non-object-schema in `AllOfSchema`")
                .lookup(key)
            {
                return Some(v);
            }
        }

        None
    }

    /// Lookup the schema for a key.
    ///
    /// The `get_type` callback will be used when descending into [`OneOfSchema`]s to decide which
    /// variant is the currently active one.
    ///
    /// Otherwise, [`OneOfSchema`]s only return the schema for their type property, but their
    /// variants are ignored.
    pub fn lookup_schema(
        &self,
        key: &str,
        get_type: &dyn WithTypeField,
    ) -> Option<(bool, &'static Schema)> {
        for entry in self.list {
            if let Some(v) = entry
                .any_object()
                .expect("non-object-schema in `AllOfSchema`")
                .lookup_schema(key, get_type)
            {
                return Some(v);
            }
        }

        None
    }

    /// Parse key/value pairs and verify with object schema
    ///
    /// - `test_required`: is set, checks if all required properties are
    ///   present.
    pub fn parse_parameter_strings(
        &'static self,
        data: &[(String, String)],
        test_required: bool,
    ) -> Result<Value, ParameterError> {
        ParameterSchema::from(self).parse_parameter_strings(data, test_required)
    }

    /// Check whether the schema contains a specific parameter.
    /// This is meant for compile time checks and not as public API.
    #[doc(hidden)]
    pub const fn const_has_parameter(&self, name: &[u8], additional_properties: bool) -> bool {
        let mut i = 0;
        while i < self.list.len() {
            if self.list[i].const_has_parameter(name, additional_properties) {
                return true;
            }
            i += 1;
        }
        false
    }
}

/// An object schema which is basically like a rust enum: exactly one variant may match.
///
/// Contrary to JSON Schema, we require there be a 'type' property to distinguish the types.
/// In serde-language, we use an internally tagged enum representation.
///
/// Note that these are limited to object schemas. Other schemas will produce errors.
#[derive(Debug)]
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
#[non_exhaustive]
pub struct OneOfSchema {
    pub description: &'static str,

    /// The type property entry.
    ///
    /// This must be a static reference due to how we implemented the property iterator.
    pub type_property_entry: &'static SchemaPropertyEntry,

    /// The parameter is checked against all of the schemas in the list. Note that all schemas must
    /// be object schemas.
    pub list: &'static [(&'static str, &'static Schema)],
}

const fn assert_one_of_list_is_sorted(list: &[(&str, &Schema)]) {
    use std::cmp::Ordering;

    let mut i = 0;
    let mut prev = None::<&'static str>;
    while i != list.len() {
        let cur = list[i].0;
        if let Some(prev) = prev {
            match proxmox_const_utils::byte_string_cmp(prev.as_bytes(), cur.as_bytes()) {
                Ordering::Greater => panic!("oneOf variant list must be sorted"),
                Ordering::Equal => panic!("multiple variants of the same type"),
                Ordering::Less => (),
            }
        }
        prev = Some(cur);
        i += 1;
    }
}

const fn assert_only_objects_in_one_of(list: &'static [(&'static str, &'static Schema)]) {
    let mut i = 0;
    while i != list.len() {
        match list[i].1 {
            Schema::Object(_) => (),
            Schema::AllOf(sub) => assert_only_objects_in_all_of(sub.list),
            Schema::OneOf(sub) => assert_only_objects_in_one_of(sub.list),
            _ => panic!("non-object schema in one-of schema"),
        }
        i += 1;
    }
}

const fn assert_no_variant_contains(list: &'static [(&'static str, &'static Schema)], name: &str) {
    let name = name.as_bytes();
    let mut i = 0;
    while i != list.len() {
        if list[i].1.const_has_parameter(name, false) {
            panic!("a oneOf variant contains a property with the same name as the type property");
        }
        i += 1;
    }
}

impl OneOfSchema {
    /// Create a new `oneOf` schema.
    ///
    /// Note that the `properties` must be sorted! Therefore it is recommended that any schema
    /// created outside of a `const` context is wrapped in a `const {}` block.
    /// (The `#[sortable]` attribute can be used to automate the sorting).
    ///
    /// The type property reuses the same tuple type as object properties, however, it must not be
    /// marked as optional.
    ///
    /// This is okay:
    ///
    /// ```
    /// # use proxmox_schema::{OneOfSchema, ObjectSchema, Schema, StringSchema};
    /// # const SCHEMA_V1: Schema = ObjectSchema::new(
    /// #     "Some Object",
    /// #     &[
    /// #         ("key1", false, &StringSchema::new("A String").schema()),
    /// #         ("key2", false, &StringSchema::new("Another String").schema()),
    /// #     ],
    /// # ).schema();
    /// # const SCHEMA_V2: Schema = ObjectSchema::new(
    /// #     "Another Object",
    /// #     &[
    /// #         ("key3", false, &StringSchema::new("A String").schema()),
    /// #         ("key4", false, &StringSchema::new("Another String").schema()),
    /// #     ],
    /// # ).schema();
    /// const SCHEMA: Schema = OneOfSchema::new(
    ///     "Some enum",
    ///     &("type", false, &StringSchema::new("v1 or v2").schema()),
    ///     &[
    ///         ("v1", &SCHEMA_V1),
    ///         ("v2", &SCHEMA_V2),
    ///     ],
    /// ).schema();
    /// ```
    ///
    /// These will panic:
    ///
    /// ```compile_fail,E0080
    /// # use proxmox_schema::{OneOfSchema, ObjectSchema, Schema, StringSchema};
    /// # const SCHEMA_V1: Schema = ObjectSchema::new(
    /// #     "Some Object",
    /// #     &[
    /// #         ("key1", false, &StringSchema::new("A String").schema()),
    /// #         ("key2", false, &StringSchema::new("Another String").schema()),
    /// #     ],
    /// # ).schema();
    /// # const SCHEMA_V2: Schema = ObjectSchema::new(
    /// #     "Another Object",
    /// #     &[
    /// #         ("key3", false, &StringSchema::new("A String").schema()),
    /// #         ("key4", false, &StringSchema::new("Another String").schema()),
    /// #     ],
    /// # ).schema();
    /// const SCHEMA: Schema = OneOfSchema::new(
    ///     "Some enum",
    ///     &("type", false, &StringSchema::new("v1 or v2").schema()),
    ///     &[
    ///         ("v2", &SCHEMA_V1),
    ///         ("v1", &SCHEMA_V2),
    ///     ],
    /// ).schema();
    /// ```
    ///
    /// ```compile_fail,E0080
    /// # use proxmox_schema::{OneOfSchema, ObjectSchema, Schema, StringSchema};
    /// # const SCHEMA_V1: Schema = ObjectSchema::new(
    /// #     "Some Object",
    /// #     &[
    /// #         ("key1", false, &StringSchema::new("A String").schema()),
    /// #         ("key2", false, &StringSchema::new("Another String").schema()),
    /// #     ],
    /// # ).schema();
    /// # const SCHEMA_V2: Schema = ObjectSchema::new(
    /// #     "Another Object",
    /// #     &[
    /// #         ("key3", false, &StringSchema::new("A String").schema()),
    /// #         ("key4", false, &StringSchema::new("Another String").schema()),
    /// #     ],
    /// # ).schema();
    /// const SCHEMA: Schema = OneOfSchema::new(
    ///     "Some enum",
    ///     &("type", false, &StringSchema::new("v1 or v2").schema()),
    ///     &[
    ///         ("v1", &SCHEMA_V1),
    ///         // duplicate type key:
    ///         ("v1", &SCHEMA_V2),
    ///     ],
    /// ).schema();
    /// ```
    ///
    /// ```compile_fail,E0080
    /// # use proxmox_schema::{OneOfSchema, ObjectSchema, Schema, StringSchema};
    /// # const SCHEMA_V1: Schema = ObjectSchema::new(
    /// #     "Some Object",
    /// #     &[
    /// #         ("key1", false, &StringSchema::new("A String").schema()),
    /// #         ("key2", false, &StringSchema::new("Another String").schema()),
    /// #     ],
    /// # ).schema();
    /// # const SCHEMA_V2: Schema = ObjectSchema::new(
    /// #     "Another Object",
    /// #     &[
    /// #         ("key3", false, &StringSchema::new("A String").schema()),
    /// #         ("key4", false, &StringSchema::new("Another String").schema()),
    /// #     ],
    /// # ).schema();
    /// const SCHEMA: Schema = OneOfSchema::new(
    ///     "Some enum",
    ///     &("type", true, &StringSchema::new("v1 or v2").schema()),
    ///     &[
    ///         ("v1", &SCHEMA_V1),
    ///         ("v2", &SCHEMA_V2),
    ///     ],
    /// ).schema();
    /// ```
    pub const fn new(
        description: &'static str,
        type_property_entry: &'static SchemaPropertyEntry,
        list: &'static [(&'static str, &'static Schema)],
    ) -> Self {
        assert_one_of_list_is_sorted(list);
        assert_only_objects_in_one_of(list);
        assert_no_variant_contains(list, type_property_entry.0);
        assert!(!type_property_entry.1, "type property must not be optional");
        Self {
            description,
            type_property_entry,
            list,
        }
    }

    pub const fn schema(self) -> Schema {
        Schema::OneOf(self)
    }

    pub fn type_property(&self) -> &'static str {
        self.type_property_entry.0
    }

    pub fn type_schema(&self) -> &'static Schema {
        self.type_property_entry.2
    }

    pub fn lookup(&self, key: &str) -> Option<(bool, &'static Schema)> {
        if key == self.type_property() {
            return Some((false, self.type_schema()));
        }

        for (_variant, entry) in self.list {
            if let Some(v) = entry
                .any_object()
                .expect("non-object-schema in `OneOfSchema`")
                .lookup(key)
            {
                return Some(v);
            }
        }

        None
    }

    /// Lookup the schema for a key.
    ///
    /// The `get_type` callback will be used when descending into [`OneOfSchema`]s to decide which
    /// variant is the currently active one.
    ///
    /// Otherwise, [`OneOfSchema`]s only return the schema for their type property, but their
    /// variants are ignored.
    pub fn lookup_schema(
        &self,
        key: &str,
        get_type: &dyn WithTypeField,
    ) -> Option<(bool, &'static Schema)> {
        if key == self.type_property() {
            return Some((self.type_property_entry.1, self.type_property_entry.2));
        }

        let mut out = None;
        get_type.with_type_field(self.type_property(), &mut |variant| {
            let Some(variant) = variant else { return };

            if let Some(schema) = self.lookup_variant(variant) {
                out = schema
                    .any_object()
                    .expect("non-object-schema in `OneOfSchema`")
                    .lookup_schema(key, get_type)
            }
        });
        out
    }

    pub fn lookup_variant(&self, name: &str) -> Option<&'static Schema> {
        Some(
            self.list[self
                .list
                .binary_search_by_key(&name, |(name, _)| name)
                .ok()?]
            .1,
        )
    }

    /// Parse key/value pairs and verify with object schema
    ///
    /// - `test_required`: is set, checks if all required properties are
    ///   present.
    pub fn parse_parameter_strings(
        &'static self,
        data: &[(String, String)],
        test_required: bool,
    ) -> Result<Value, ParameterError> {
        ParameterSchema::from(self).parse_parameter_strings(data, test_required)
    }

    /// Check whether the schema contains a specific parameter.
    /// This is meant for compile time checks and not as public API.
    #[doc(hidden)]
    pub const fn const_has_parameter(&self, name: &[u8], additional_properties: bool) -> bool {
        if proxmox_const_utils::byte_string_eq(self.type_property_entry.0.as_bytes(), name) {
            return true;
        }
        let mut i = 0;
        while i < self.list.len() {
            if self.list[i]
                .1
                .const_has_parameter(name, additional_properties)
            {
                return true;
            }
            i += 1;
        }
        false
    }
}

mod private {
    pub trait Sealed: Send + Sync {}
    impl Sealed for super::ObjectSchema {}
    impl Sealed for super::AllOfSchema {}
    impl Sealed for super::OneOfSchema {}
    impl Sealed for super::ParameterSchema {}
}

/// Check `aliases_present` for conflicts and, if there are none, rename each alias key in `map`
/// to its canonical name. The caller guarantees each `alias_key` is already present in `map`. A
/// conflict (an alias and its canonical both set, or two different aliases of the same canonical)
/// leaves `map` unchanged and returns a [`ParameterError`].
fn rename_aliases_to_canonicals(
    map: &mut serde_json::Map<String, Value>,
    aliases_present: &[(&str, &'static str)],
) -> Result<(), Error> {
    let mut errors = ParameterError::new();
    let mut seen_canonical: HashSet<&'static str> = HashSet::new();
    for (alias_key, canonical) in aliases_present {
        if map.contains_key(*canonical) {
            errors.push(
                alias_key.to_string(),
                format_err!("cannot set both `{canonical}` and `{alias_key}`."),
            );
        } else if !seen_canonical.insert(canonical) {
            errors.push(
                alias_key.to_string(),
                format_err!("cannot set both `{alias_key}` and another alias of `{canonical}`."),
            );
        }
    }
    if !errors.is_empty() {
        return Err(errors.into());
    }
    for (alias_key, canonical) in aliases_present {
        let v = map.remove(*alias_key).expect("alias key was just observed");
        map.insert(canonical.to_string(), v);
    }
    Ok(())
}

/// Beside [`ObjectSchema`] we also have an [`AllOfSchema`] which also represents objects.
pub trait ObjectSchemaType: private::Sealed + Send + Sync {
    fn description(&self) -> &'static str;
    fn lookup(&self, key: &str) -> Option<(bool, &'static Schema)>;
    fn properties(&self) -> ObjectPropertyIterator;
    fn additional_properties(&self) -> bool;
    fn default_key(&self) -> Option<&'static str>;

    /// Resolve a deprecated property name to its canonical name.
    ///
    /// Schemas that have no aliases (the common case) inherit the default `None` here.
    fn lookup_alias(&self, _key: &str) -> Option<&'static str> {
        None
    }

    /// Lookup the schema for a key.
    ///
    /// The `get_type` callback will be used when descending into [`OneOfSchema`]s to decide which
    /// variant is the currently active one.
    ///
    /// Otherwise, [`OneOfSchema`]s only return the schema for their type property, but their
    /// variants are ignored.
    fn lookup_schema(
        &self,
        key: &str,
        get_type: &dyn WithTypeField,
    ) -> Option<(bool, &'static Schema)>;

    /// Rewrite deprecated alias keys in `value` to their canonical names.
    ///
    /// For object values, every key registered via [`ObjectSchema::property_aliases`] is renamed
    /// to its canonical name. If both an alias and its canonical (or two different aliases of
    /// the same canonical) are present, a `cannot set both` [`ParameterError`] is returned and
    /// `value` is left unchanged.
    ///
    /// Callers handing the resulting [`Value`] to API handlers generated by `#[api]` (which look
    /// up properties by canonical name only) must call this before verification and dispatch;
    /// otherwise the alias key passes [`verify_json`](Self::verify_json) but the downstream
    /// handler reports the canonical as missing.
    ///
    /// The default implementation walks `value.keys()` and resolves each through
    /// [`lookup_alias`](Self::lookup_alias); concrete schema kinds that can find their alias
    /// list more directly (such as [`ObjectSchema`]'s static sidecar) override this. No-op
    /// for schemas with no declared aliases and for non-object values.
    fn canonicalize_aliases(&self, value: &mut Value) -> Result<(), Error>;

    /// Should always return `None`, unless dealing with *legacy* PVE property strings.
    fn key_alias_info(&self) -> Option<KeyAliasInfo> {
        None
    }

    /// Verify JSON value using an object schema.
    fn verify_json(&self, data: &Value) -> Result<(), Error> {
        let map = match data {
            Value::Object(map) => map,
            Value::Array(_) => bail!("Expected object - got array."),
            _ => bail!("Expected object - got scalar value."),
        };

        let mut errors = ParameterError::new();

        let additional_properties = self.additional_properties();

        // Track canonical names already seen via an alias to also catch two distinct aliases
        // of the same canonical being set at once. Callers going through
        // [`canonicalize_aliases`](Self::canonicalize_aliases) before `verify_json` find no
        // aliases left in the map and skip these branches entirely.
        let mut seen_alias_canonical: HashSet<&'static str> = HashSet::new();
        for (key, value) in map {
            if let Some(canonical) = self.lookup_alias(key) {
                if map.contains_key(canonical) {
                    errors.push(
                        key.to_string(),
                        format_err!("cannot set both `{canonical}` and `{key}`."),
                    );
                    continue;
                }
                if !seen_alias_canonical.insert(canonical) {
                    errors.push(
                        key.to_string(),
                        format_err!("cannot set both `{key}` and another alias of `{canonical}`."),
                    );
                    continue;
                }
            }
            if let Some((_optional, prop_schema)) = self.lookup(key) {
                if let Err(err) = prop_schema.verify_json(value) {
                    errors.add_errors(key, err);
                };
            } else if !additional_properties {
                errors.push(
                    key.to_string(),
                    format_err!("schema does not allow additional properties"),
                );
            }
        }

        for (name, optional, _prop_schema) in self.properties() {
            if *optional || data[name] != Value::Null {
                continue;
            }
            // a present alias satisfies the required-property check
            if map
                .keys()
                .filter_map(|k| self.lookup_alias(k))
                .any(|canon| canon == *name)
            {
                continue;
            }
            errors.push(
                name.to_string(),
                format_err!("property is missing and it is not optional"),
            );
        }

        if !errors.is_empty() {
            Err(errors.into())
        } else {
            Ok(())
        }
    }
}

#[doc(hidden)]
pub enum ObjectPropertyIterator {
    Simple(SimpleObjectPropertyIterator),
    OneOf(OneOfPropertyIterator),
}

impl Iterator for ObjectPropertyIterator {
    type Item = &'static SchemaPropertyEntry;

    fn next(&mut self) -> Option<&'static SchemaPropertyEntry> {
        match self {
            Self::Simple(iter) => iter.next(),
            Self::OneOf(iter) => iter.next(),
        }
    }
}

impl ObjectSchemaType for ObjectSchema {
    fn description(&self) -> &'static str {
        self.description
    }

    fn lookup(&self, key: &str) -> Option<(bool, &'static Schema)> {
        ObjectSchema::lookup(self, key)
    }

    fn lookup_schema(&self, key: &str, _: &dyn WithTypeField) -> Option<(bool, &'static Schema)> {
        ObjectSchema::lookup(self, key)
    }

    fn properties(&self) -> ObjectPropertyIterator {
        ObjectPropertyIterator::Simple(SimpleObjectPropertyIterator {
            schemas: [].iter(),
            properties: Some(self.properties.iter()),
            nested: None,
        })
    }

    fn additional_properties(&self) -> bool {
        self.additional_properties
    }

    fn default_key(&self) -> Option<&'static str> {
        self.default_key
    }

    fn lookup_alias(&self, key: &str) -> Option<&'static str> {
        ObjectSchema::lookup_alias(self, key)
    }

    /// Works directly from the static [`property_aliases`](ObjectSchema::property_aliases) list
    /// so schemas without aliases (the common case) skip the map walk entirely.
    fn canonicalize_aliases(&self, value: &mut Value) -> Result<(), Error> {
        if self.property_aliases.is_empty() {
            return Ok(());
        }
        let Some(map) = value.as_object_mut() else {
            return Ok(());
        };
        let aliases: Vec<(&str, &'static str)> = self
            .property_aliases
            .iter()
            .filter(|&&(alias, _canonical)| map.contains_key(alias))
            .map(|&(alias, canonical)| (alias, canonical))
            .collect();
        if aliases.is_empty() {
            return Ok(());
        }
        rename_aliases_to_canonicals(map, &aliases)
    }

    fn key_alias_info(&self) -> Option<KeyAliasInfo> {
        self.key_alias_info
    }
}

impl ObjectSchemaType for AllOfSchema {
    fn description(&self) -> &'static str {
        self.description
    }

    fn lookup(&self, key: &str) -> Option<(bool, &'static Schema)> {
        AllOfSchema::lookup(self, key)
    }

    fn lookup_schema(
        &self,
        key: &str,
        get_type: &dyn WithTypeField,
    ) -> Option<(bool, &'static Schema)> {
        AllOfSchema::lookup_schema(self, key, get_type)
    }

    fn properties(&self) -> ObjectPropertyIterator {
        ObjectPropertyIterator::Simple(SimpleObjectPropertyIterator {
            schemas: self.list.iter(),
            properties: None,
            nested: None,
        })
    }

    fn additional_properties(&self) -> bool {
        self.list.iter().any(|schema| {
            schema
                .any_object()
                .expect("non-object-schema in `AllOfSchema`")
                .additional_properties()
        })
    }

    fn default_key(&self) -> Option<&'static str> {
        for schema in self.list {
            let default_key = schema
                .any_object()
                .expect("non-object-schema in `AllOfSchema`")
                .default_key();

            if default_key.is_some() {
                return default_key;
            }
        }

        None
    }

    /// Iterates branches left-to-right and returns the first match (`find_map`). Branches that
    /// declare conflicting alias targets for the same alias name are not detected here; in
    /// practice this is unlikely to come up, since AllOf branches are usually disjoint by
    /// design.
    fn lookup_alias(&self, key: &str) -> Option<&'static str> {
        self.list.iter().find_map(|schema| {
            schema
                .any_object()
                .expect("non-object-schema in `AllOfSchema`")
                .lookup_alias(key)
        })
    }

    /// Forwards to each sub-schema so the specialized per-schema-kind logic runs (e.g. an
    /// inner [`ObjectSchema`] uses its sidecar and skips schemas without aliases).
    fn canonicalize_aliases(&self, value: &mut Value) -> Result<(), Error> {
        for schema in self.list {
            schema
                .any_object()
                .expect("non-object-schema in `AllOfSchema`")
                .canonicalize_aliases(value)?;
        }
        Ok(())
    }
}

#[doc(hidden)]
pub struct SimpleObjectPropertyIterator {
    schemas: std::slice::Iter<'static, &'static Schema>,
    properties: Option<std::slice::Iter<'static, SchemaPropertyEntry>>,
    nested: Option<Box<ObjectPropertyIterator>>,
}

impl Iterator for SimpleObjectPropertyIterator {
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
                    Schema::OneOf(o) => self.nested = Some(Box::new(o.properties())),
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

impl ObjectSchemaType for OneOfSchema {
    fn description(&self) -> &'static str {
        self.description
    }

    fn lookup(&self, key: &str) -> Option<(bool, &'static Schema)> {
        OneOfSchema::lookup(self, key)
    }

    fn lookup_schema(
        &self,
        key: &str,
        get_type: &dyn WithTypeField,
    ) -> Option<(bool, &'static Schema)> {
        OneOfSchema::lookup_schema(self, key, get_type)
    }

    fn properties(&self) -> ObjectPropertyIterator {
        ObjectPropertyIterator::OneOf(OneOfPropertyIterator {
            type_property_entry: self.type_property_entry,
            schemas: self.list.iter(),
            done: HashSet::new(),
            nested: None,
        })
    }

    fn additional_properties(&self) -> bool {
        self.list.iter().any(|(_, schema)| {
            schema
                .any_object()
                .expect("non-object-schema in `OneOfSchema`")
                .additional_properties()
        })
    }

    fn default_key(&self) -> Option<&'static str> {
        None
    }

    fn lookup_alias(&self, key: &str) -> Option<&'static str> {
        self.list.iter().find_map(|(_, schema)| match schema {
            Schema::String(_) => None,
            _ => schema
                .any_object()
                .expect("non-object-schema in `OneOfSchema`")
                .lookup_alias(key),
        })
    }

    /// Forwards to each sub-schema so the specialized per-schema-kind logic runs (e.g. an
    /// inner [`ObjectSchema`] uses its sidecar and skips schemas without aliases).
    fn canonicalize_aliases(&self, value: &mut Value) -> Result<(), Error> {
        for (_, schema) in self.list {
            // a oneOf may carry a single plain-string variant; it has no properties to
            // canonicalize, so skip it like `lookup_alias` and `additional_properties` do.
            match schema {
                Schema::String(_) => continue,
                _ => schema
                    .any_object()
                    .expect("non-object-schema in `OneOfSchema`")
                    .canonicalize_aliases(value)?,
            }
        }
        Ok(())
    }

    fn verify_json(&self, data: &Value) -> Result<(), Error> {
        let map = match data {
            Value::Object(map) => map,
            Value::Array(_) => bail!("Expected object - got array."),
            _ => bail!("Expected object - got scalar value."),
        };

        // Without the type we also cannot verify anything else...:
        let variant = match map.get(self.type_property()) {
            None => bail!("Missing '{}' property", self.type_property()),
            Some(Value::String(v)) => v,
            _ => bail!("Expected string in '{}'", self.type_property()),
        };

        let schema = self
            .lookup_variant(variant)
            .ok_or_else(|| format_err!("invalid '{}': {}", self.type_property(), variant))?;

        schema.verify_json(data)
    }
}

#[doc(hidden)]
pub struct OneOfPropertyIterator {
    type_property_entry: &'static SchemaPropertyEntry,
    schemas: std::slice::Iter<'static, (&'static str, &'static Schema)>,
    done: HashSet<&'static str>,
    nested: Option<Box<ObjectPropertyIterator>>,
}

impl Iterator for OneOfPropertyIterator {
    type Item = &'static SchemaPropertyEntry;

    fn next(&mut self) -> Option<&'static SchemaPropertyEntry> {
        if self.done.insert(self.type_property_entry.0) {
            return Some(self.type_property_entry);
        }

        loop {
            match self.nested.as_mut().and_then(Iterator::next) {
                Some(item) => {
                    if !self.done.insert(item.0) {
                        continue;
                    }
                    return Some(item);
                }
                None => self.nested = None,
            }

            self.nested = Some(Box::new(
                self.schemas
                    .next()?
                    .1
                    .any_object()
                    .expect("non-object-schema in `OneOfSchema`")
                    .properties(),
            ));
        }
    }
}

/// Schemas are used to describe complex data types.
///
/// All schema types implement constant builder methods, and a final
/// `schema()` method to convert them into a `Schema`.
///
/// ```
/// use proxmox_schema::{Schema, BooleanSchema, IntegerSchema, ObjectSchema};
///
/// const SIMPLE_OBJECT: Schema = ObjectSchema::new(
///     "A very simple object with 2 properties",
///     &[ // this arrays needs to be sorted by name!
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
    OneOf(OneOfSchema),
}

impl Schema {
    /// Verify JSON value with `schema`.
    pub fn verify_json(&self, data: &Value) -> Result<(), Error> {
        match self {
            Schema::Null => {
                if !data.is_null() {
                    bail!("Expected Null, but value is not Null.");
                }
            }
            Schema::Object(s) => s.verify_json(data)?,
            Schema::Array(s) => s.verify_json(data)?,
            Schema::Boolean(s) => s.verify_json(data)?,
            Schema::Integer(s) => s.verify_json(data)?,
            Schema::Number(s) => s.verify_json(data)?,
            Schema::String(s) => s.verify_json(data)?,
            Schema::AllOf(s) => s.verify_json(data)?,
            Schema::OneOf(s) => s.verify_json(data)?,
        }
        Ok(())
    }

    /// Parse a simple value (no arrays and no objects)
    pub fn parse_simple_value(&self, value_str: &str) -> Result<Value, Error> {
        let value = match self {
            Schema::Null => {
                bail!("internal error - found Null schema.");
            }
            Schema::Boolean(_boolean_schema) => {
                let res = parse_boolean(value_str)?;
                Value::Bool(res)
            }
            Schema::Integer(integer_schema) => {
                let res: i64 = value_str.parse()?;
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

    /// Parse a complex property string (`ApiStringFormat::PropertyString`)
    pub fn parse_property_string(&'static self, value_str: &str) -> Result<Value, Error> {
        // helper for object/allof schemas:
        fn parse_object<T: Into<ParameterSchema>>(
            value_str: &str,
            schema: T,
            default_key: Option<&'static str>,
        ) -> Result<Value, Error> {
            let mut param_list = Vec::new();
            for entry in crate::property_string::PropertyIterator::new(value_str) {
                let (key, value) = entry?;
                match key {
                    Some(key) => param_list.push((key.to_string(), value.into_owned())),
                    None => {
                        if let Some(key) = default_key {
                            param_list.push((key.to_string(), value.into_owned()));
                        } else {
                            bail!(
                                "Value '{value}' without key, but schema does not define a default key."
                            );
                        }
                    }
                }
            }

            schema
                .into()
                .parse_parameter_strings(&param_list, true)
                .map_err(Error::from)
        }

        match self {
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
                    match array_schema.items.parse_simple_value(value.trim()) {
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

    /// Gets the underlying [`BooleanSchema`], panics on different schemas.
    pub const fn unwrap_boolean_schema(&self) -> &BooleanSchema {
        match self {
            Schema::Boolean(s) => s,
            _ => panic!("unwrap_boolean_schema on different schema"),
        }
    }

    /// Gets the underlying [`IntegerSchema`], panics on different schemas.
    pub const fn unwrap_integer_schema(&self) -> &IntegerSchema {
        match self {
            Schema::Integer(s) => s,
            _ => panic!("unwrap_integer_schema on different schema"),
        }
    }

    /// Gets the underlying [`NumberSchema`], panics on different schemas.
    pub const fn unwrap_number_schema(&self) -> &NumberSchema {
        match self {
            Schema::Number(s) => s,
            _ => panic!("unwrap_number_schema on different schema"),
        }
    }

    /// Gets the underlying [`StringSchema`], panics on different schemas.
    pub const fn unwrap_string_schema(&self) -> &StringSchema {
        match self {
            Schema::String(s) => s,
            _ => panic!("unwrap_string_schema on different schema"),
        }
    }

    /// Gets the underlying [`ObjectSchema`], panics on different schemas.
    pub const fn unwrap_object_schema(&self) -> &ObjectSchema {
        match self {
            Schema::Object(s) => s,
            _ => panic!("unwrap_object_schema on different schema"),
        }
    }

    /// Gets the underlying [`ArraySchema`], panics on different schemas.
    pub const fn unwrap_array_schema(&self) -> &ArraySchema {
        match self {
            Schema::Array(s) => s,
            _ => panic!("unwrap_array_schema on different schema"),
        }
    }

    /// Gets the underlying [`AllOfSchema`], panics on different schemas.
    pub const fn unwrap_all_of_schema(&self) -> &AllOfSchema {
        match self {
            Schema::AllOf(s) => s,
            _ => panic!("unwrap_all_of_schema on different schema"),
        }
    }

    /// Gets the underlying [`OneOfSchema`], panics on different schemas.
    pub const fn unwrap_one_of_schema(&self) -> &OneOfSchema {
        match self {
            Schema::OneOf(s) => s,
            _ => panic!("unwrap_one_of_schema on different schema"),
        }
    }

    /// Gets the underlying schema as a [`dyn ObjectSchemaType`], panics on schema types other than
    /// `Object`, `AllOf` or `OneOf`.
    pub const fn unwrap_any_object_schema(&self) -> &dyn ObjectSchemaType {
        match self {
            Schema::Object(s) => s,
            Schema::AllOf(s) => s,
            Schema::OneOf(s) => s,
            _ => panic!("unwrap_any_object_schema on non-object schema"),
        }
    }

    /// Gets the underlying [`BooleanSchema`].
    pub const fn boolean(&self) -> Option<&BooleanSchema> {
        match self {
            Schema::Boolean(s) => Some(s),
            _ => None,
        }
    }

    /// Gets the underlying [`IntegerSchema`].
    pub const fn integer(&self) -> Option<&IntegerSchema> {
        match self {
            Schema::Integer(s) => Some(s),
            _ => None,
        }
    }

    /// Gets the underlying [`NumberSchema`].
    pub const fn number(&self) -> Option<&NumberSchema> {
        match self {
            Schema::Number(s) => Some(s),
            _ => None,
        }
    }

    /// Gets the underlying [`StringSchema`].
    pub const fn string(&self) -> Option<&StringSchema> {
        match self {
            Schema::String(s) => Some(s),
            _ => None,
        }
    }

    /// Gets the underlying [`ObjectSchema`].
    pub const fn object(&self) -> Option<&ObjectSchema> {
        match self {
            Schema::Object(s) => Some(s),
            _ => None,
        }
    }

    /// Gets the underlying [`ArraySchema`].
    pub const fn array(&self) -> Option<&ArraySchema> {
        match self {
            Schema::Array(s) => Some(s),
            _ => None,
        }
    }

    /// Gets the underlying [`AllOfSchema`].
    pub const fn all_of(&self) -> Option<&AllOfSchema> {
        match self {
            Schema::AllOf(s) => Some(s),
            _ => None,
        }
    }

    /// Gets the underlying [`AllOfSchema`].
    pub const fn one_of(&self) -> Option<&OneOfSchema> {
        match self {
            Schema::OneOf(s) => Some(s),
            _ => None,
        }
    }

    /// Gets the underlying schema as a [`dyn ObjectSchemaType`].
    pub const fn any_object(&self) -> Option<&dyn ObjectSchemaType> {
        match self {
            Schema::Object(s) => Some(s),
            Schema::AllOf(s) => Some(s),
            Schema::OneOf(s) => Some(s),
            _ => None,
        }
    }

    /// Gets a copy of the underlying [`BooleanSchema`], panics on different schemas.
    pub const fn unwrap_boolean_schema_cloned(&self) -> BooleanSchema {
        match self {
            Schema::Boolean(s) => BooleanSchema { ..*s },
            _ => panic!("unwrap_boolean_schema_cloned on different schema"),
        }
    }

    /// Gets a copy of the underlying [`IntegerSchema`], panics on different schemas.
    pub const fn unwrap_integer_schema_cloned(&self) -> IntegerSchema {
        match self {
            Schema::Integer(s) => IntegerSchema { ..*s },
            _ => panic!("unwrap_integer_schema_cloned on different schema"),
        }
    }

    /// Gets a copy of the underlying [`NumberSchema`], panics on different schemas.
    pub const fn unwrap_number_schema_cloned(&self) -> NumberSchema {
        match self {
            Schema::Number(s) => NumberSchema { ..*s },
            _ => panic!("unwrap_number_schema_cloned on different schema"),
        }
    }

    /// Gets the underlying [`StringSchema`], panics on different schemas.
    pub const fn unwrap_string_schema_cloned(&self) -> StringSchema {
        match self {
            Schema::String(s) => StringSchema { ..*s },
            _ => panic!("unwrap_string_schema_cloned on different schema"),
        }
    }

    /// Gets the underlying [`ObjectSchema`], panics on different schemas.
    pub const fn unwrap_object_schema_cloned(&self) -> ObjectSchema {
        match self {
            Schema::Object(s) => ObjectSchema { ..*s },
            _ => panic!("unwrap_object_schema_cloned on different schema"),
        }
    }

    /// Gets the underlying [`ArraySchema`], panics on different schemas.
    pub const fn unwrap_array_schema_cloned(&self) -> ArraySchema {
        match self {
            Schema::Array(s) => ArraySchema { ..*s },
            _ => panic!("unwrap_array_schema_cloned on different schema"),
        }
    }

    /// Gets the underlying [`AllOfSchema`], panics on different schemas.
    pub const fn unwrap_all_of_schema_cloned(&self) -> AllOfSchema {
        match self {
            Schema::AllOf(s) => AllOfSchema { ..*s },
            _ => panic!("unwrap_all_of_schema_cloned on different schema"),
        }
    }

    /// Gets the underlying [`OneOfSchema`], panics on different schemas.
    pub const fn unwrap_one_of_schema_cloned(&self) -> OneOfSchema {
        match self {
            Schema::OneOf(s) => OneOfSchema { ..*s },
            _ => panic!("unwrap_one_of_schema_cloned on different schema"),
        }
    }

    /// Check whether the schema contains a specific parameter.
    /// This is meant for compile time checks and not as public API.
    #[doc(hidden)]
    pub const fn const_has_parameter(&self, name: &[u8], additional_properties: bool) -> bool {
        match self {
            Schema::Object(object) => object.const_has_parameter(name, additional_properties),
            Schema::AllOf(all_of) => all_of.const_has_parameter(name, additional_properties),
            Schema::OneOf(one_of) => one_of.const_has_parameter(name, additional_properties),
            _ => false,
        }
    }
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
/// use proxmox_schema::{ApiStringFormat, EnumEntry};
///
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
/// use proxmox_schema::{const_regex, ApiStringFormat};
///
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
/// list may be separated by comma, semicolon or whitespace.
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
/// use proxmox_schema::{ApiStringFormat, ArraySchema, IntegerSchema, Schema, StringSchema};
///
/// const PRODUCT_LIST_SCHEMA: Schema =
///             ArraySchema::new("Product List.", &IntegerSchema::new("Product ID").schema())
///                 .min_length(1)
///                 .schema();
///
/// const SCHEMA: Schema = StringSchema::new("A list of Product IDs, comma separated.")
///     .format(&ApiStringFormat::PropertyString(&PRODUCT_LIST_SCHEMA))
///     .schema();
///
/// let res = SCHEMA.parse_simple_value("");
/// assert!(res.is_err());
///
/// let res = SCHEMA.parse_simple_value("1,2,3"); // parse as String
/// assert!(res.is_ok());
///
/// let data = PRODUCT_LIST_SCHEMA.parse_property_string("1,2"); // parse as Array
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
    VerifyFn(ApiStringVerifyFn),
}

/// Type of a verification function for [`StringSchema`]s.
pub type ApiStringVerifyFn = fn(&str) -> Result<(), Error>;

impl ApiStringFormat {
    /// Gets the underlying [`&[EnumEntry]`](EnumEntry) list, panics on different formats.
    pub const fn unwrap_enum_format(&self) -> &'static [EnumEntry] {
        match self {
            ApiStringFormat::Enum(v) => v,
            _ => panic!("unwrap_enum_format on a different ApiStringFormat"),
        }
    }

    /// Gets the underlying [`&ConstRegexPattern`](ConstRegexPattern), panics on different formats.
    pub const fn unwrap_pattern_format(&self) -> &'static ConstRegexPattern {
        match self {
            ApiStringFormat::Pattern(v) => v,
            _ => panic!("unwrap_pattern_format on a different ApiStringFormat"),
        }
    }

    /// Gets the underlying property [`&Schema`](Schema), panics on different formats.
    pub const fn unwrap_property_string_format(&self) -> &'static Schema {
        match self {
            ApiStringFormat::PropertyString(v) => v,
            _ => panic!("unwrap_property_string_format on a different ApiStringFormat"),
        }
    }
}

impl std::fmt::Debug for ApiStringFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ApiStringFormat::VerifyFn(fnptr) => write!(f, "VerifyFn({fnptr:p}"),
            ApiStringFormat::Enum(variants) => write!(f, "Enum({variants:?}"),
            ApiStringFormat::Pattern(regex) => write!(f, "Pattern({regex:?}"),
            ApiStringFormat::PropertyString(schema) => write!(f, "PropertyString({schema:?}"),
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
    OneOf(&'static OneOfSchema),
}

impl ParameterSchema {
    /// Parse key/value pairs and verify with object schema
    ///
    /// - `test_required`: is set, checks if all required properties are
    ///   present.
    pub fn parse_parameter_strings(
        self,
        data: &[(String, String)],
        test_required: bool,
    ) -> Result<Value, ParameterError> {
        do_parse_parameter_strings(self, data, test_required)
    }

    /// Check whether the schema contains a specific parameter.
    /// This is meant for compile time checks and not as public API.
    #[doc(hidden)]
    pub const fn const_has_parameter(&self, name: &[u8], additional_properties: bool) -> bool {
        match self {
            Self::Object(o) => o.const_has_parameter(name, additional_properties),
            Self::AllOf(o) => o.const_has_parameter(name, additional_properties),
            Self::OneOf(o) => o.const_has_parameter(name, additional_properties),
        }
    }
}

impl ObjectSchemaType for ParameterSchema {
    fn description(&self) -> &'static str {
        match self {
            ParameterSchema::Object(o) => o.description(),
            ParameterSchema::AllOf(o) => o.description(),
            ParameterSchema::OneOf(o) => o.description(),
        }
    }

    fn lookup(&self, key: &str) -> Option<(bool, &'static Schema)> {
        match self {
            ParameterSchema::Object(o) => o.lookup(key),
            ParameterSchema::AllOf(o) => o.lookup(key),
            ParameterSchema::OneOf(o) => o.lookup(key),
        }
    }

    fn lookup_schema(
        &self,
        key: &str,
        get_type: &dyn WithTypeField,
    ) -> Option<(bool, &'static Schema)> {
        match self {
            ParameterSchema::Object(o) => o.lookup_schema(key, get_type),
            ParameterSchema::AllOf(o) => o.lookup_schema(key, get_type),
            ParameterSchema::OneOf(o) => o.lookup_schema(key, get_type),
        }
    }

    fn properties(&self) -> ObjectPropertyIterator {
        match self {
            ParameterSchema::Object(o) => o.properties(),
            ParameterSchema::AllOf(o) => o.properties(),
            ParameterSchema::OneOf(o) => o.properties(),
        }
    }

    fn additional_properties(&self) -> bool {
        match self {
            ParameterSchema::Object(o) => o.additional_properties(),
            ParameterSchema::AllOf(o) => o.additional_properties(),
            ParameterSchema::OneOf(o) => o.additional_properties(),
        }
    }

    fn default_key(&self) -> Option<&'static str> {
        match self {
            ParameterSchema::Object(o) => o.default_key(),
            ParameterSchema::AllOf(o) => o.default_key(),
            ParameterSchema::OneOf(o) => o.default_key(),
        }
    }

    fn lookup_alias(&self, key: &str) -> Option<&'static str> {
        match self {
            ParameterSchema::Object(o) => o.lookup_alias(key),
            ParameterSchema::AllOf(o) => o.lookup_alias(key),
            ParameterSchema::OneOf(o) => o.lookup_alias(key),
        }
    }

    /// Dispatches to the concrete variant so the specialized impl runs (ObjectSchema's
    /// sidecar-driven fast path, AllOfSchema's per-branch forward, etc.).
    fn canonicalize_aliases(&self, value: &mut Value) -> Result<(), Error> {
        match self {
            ParameterSchema::Object(o) => o.canonicalize_aliases(value),
            ParameterSchema::AllOf(o) => o.canonicalize_aliases(value),
            ParameterSchema::OneOf(o) => o.canonicalize_aliases(value),
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

impl From<&'static OneOfSchema> for ParameterSchema {
    fn from(schema: &'static OneOfSchema) -> Self {
        ParameterSchema::OneOf(schema)
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

fn do_parse_parameter_strings(
    schema: ParameterSchema,
    data: &[(String, String)],
    test_required: bool,
) -> Result<Value, ParameterError> {
    let mut params = json!({});

    let mut errors = ParameterError::new();

    let additional_properties = schema.additional_properties();

    // Track the first raw key that wrote to each canonical key. Used to detect mixing of an
    // alias and its canonical (or two different aliases) regardless of scalar vs. array.
    let mut first_raw = HashMap::<String, String>::new();

    for (raw_key, value) in data {
        let (key, alias_from): (&str, Option<&str>) = match schema.lookup_alias(raw_key) {
            Some(canonical) => (canonical, Some(raw_key.as_str())),
            None => (raw_key.as_str(), None),
        };
        // Reject mixing an alias with its canonical (or two distinct aliases of the same
        // canonical) before any mutation. The same raw_key appearing again is *not* a mix
        // and falls through to the per-arm logic below (legitimate for arrays, rejected as
        // `duplicate parameter` for scalars).
        if let Some(prev_raw) = first_raw.get(key) {
            if prev_raw != raw_key.as_str() {
                let other = alias_from.unwrap_or(prev_raw.as_str());
                errors.push(
                    raw_key.into(),
                    format_err!("cannot set both `{key}` and `{other}`."),
                );
                continue;
            }
        } else {
            first_raw.insert(key.to_string(), raw_key.clone());
        }
        if let Some((_optional, prop_schema)) = schema.lookup(key) {
            match prop_schema {
                Schema::Array(array_schema) => {
                    if params[key] == Value::Null {
                        params[key] = json!([]);
                    }
                    match params[key] {
                        Value::Array(ref mut array) => {
                            match array_schema.items.parse_simple_value(value) {
                                Ok(res) => array.push(res), // fixme: check_length??
                                Err(err) => errors.push(raw_key.into(), err),
                            }
                        }
                        _ => errors.push(
                            raw_key.into(),
                            format_err!("expected array - type mismatch"),
                        ),
                    }
                }
                _ => match prop_schema.parse_simple_value(value) {
                    Ok(res) => {
                        if params[key] == Value::Null {
                            params[key] = res;
                        } else {
                            errors.push(raw_key.into(), format_err!("duplicate parameter."));
                        }
                    }
                    Err(err) => errors.push(raw_key.into(), err),
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
                _ => errors.push(key.into(), format_err!("expected array - type mismatch")),
            }
        } else {
            errors.push(
                key.into(),
                format_err!("schema does not allow additional properties."),
            );
        }
    }

    if test_required && errors.is_empty() {
        for (name, optional, _prop_schema) in schema.properties() {
            if *optional || params[name] != Value::Null {
                continue;
            }
            errors.push(
                name.to_string(),
                format_err!("parameter is missing and it is not optional."),
            );
        }
    }

    if !errors.is_empty() {
        Err(errors)
    } else {
        Ok(params)
    }
}

/// API types should define an "updater type" via this trait in order to support derived "Updater"
/// structs more easily.
///
/// Most trivial types can simply use an `Option<Self>` as updater. For types which do not use the
/// `#[api]` macro, this will need to be explicitly created (or derived via
/// `#[derive(UpdaterType)]`.
pub trait UpdaterType: Sized {
    type Updater: Updater;
}

#[cfg(feature = "api-macro")]
pub use proxmox_api_macro::UpdaterType;

#[cfg(feature = "api-macro")]
#[doc(hidden)]
pub use proxmox_api_macro::Updater;

macro_rules! basic_updater_type {
    ($($ty:ty)*) => {
        $(
            impl UpdaterType for $ty {
                type Updater = Option<Self>;
            }
        )*
    };
}
basic_updater_type! { bool u8 u16 u32 u64 i8 i16 i32 i64 usize isize f32 f64 String char }

impl<T> UpdaterType for Option<T>
where
    T: UpdaterType,
{
    type Updater = T::Updater;
}

// this will replace the whole Vec
impl<T> UpdaterType for Vec<T> {
    type Updater = Option<Self>;
}

impl<T> UpdaterType for crate::property_string::PropertyString<T> {
    type Updater = Option<Self>;
}

impl<K, V> UpdaterType for HashMap<K, V> {
    type Updater = Option<Self>;
}

impl<K, V> UpdaterType for BTreeMap<K, V> {
    type Updater = Option<Self>;
}

/// Trait signifying that a type contains an API schema.
pub trait ApiType {
    const API_SCHEMA: Schema;
}

impl<T: ApiType> ApiType for Option<T> {
    const API_SCHEMA: Schema = T::API_SCHEMA;
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

/// Return type schema. Return types may be any schema and additionally be optional.
#[cfg_attr(feature = "test-harness", derive(Eq, PartialEq))]
pub struct ReturnType {
    /// A return type may be optional, meaning the method may return null or some fixed data.
    ///
    /// If true, the return type in pseudo openapi terms would be `"oneOf": [ "null", "T" ]`.
    pub optional: bool,

    /// The method's return type.
    pub schema: &'static Schema,
}

impl std::fmt::Debug for ReturnType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.optional {
            write!(f, "optional {:?}", self.schema)
        } else {
            write!(f, "{:?}", self.schema)
        }
    }
}

impl ReturnType {
    pub const fn new(optional: bool, schema: &'static Schema) -> Self {
        Self { optional, schema }
    }
}
