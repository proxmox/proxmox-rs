//! Property string serialization.

use std::collections::HashMap;
use std::fmt;
use std::mem;

use serde::ser::{self, Serialize, Serializer};

use crate::de::Error;
use crate::schema::{ArraySchema, ObjectSchemaType, Schema};

impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::msg(msg.to_string())
    }
}

pub struct PropertyStringSerializer<T> {
    inner: T,
    schema: &'static Schema,
}

impl<T: fmt::Write> PropertyStringSerializer<T> {
    pub fn new(inner: T, schema: &'static Schema) -> Self {
        Self { inner, schema }
    }

    fn do_seq(self) -> Result<SerializeSeq<T>, Error> {
        let schema = self.schema.array().ok_or_else(|| {
            Error::msg("property string serializer used for array with non-array schema")
        })?;
        Ok(SerializeSeq::new(self.inner, Some(schema)))
    }

    fn do_object(self) -> Result<SerializeStruct<T>, Error> {
        SerializeStruct::try_new(self.inner, Some(self.schema))
    }
}

macro_rules! not_an_object {
    () => {};
    ($name:ident($ty:ty) $($rest:tt)*) => {
        fn $name(self, _v: $ty) -> Result<Self::Ok, Error> {
            Err(Error::msg("property string serializer used with a non-object/array type"))
        }

        not_an_object! { $($rest)* }
    };
    ($name:ident($($args:tt)*) $($rest:tt)*) => {
        fn $name(self, $($args)*) -> Result<Self::Ok, Error> {
            Err(Error::msg("property string serializer used with a non-object/array type"))
        }

        not_an_object! { $($rest)* }
    };
    ($name:ident<($($gen:tt)*)>($($args:tt)*) $($rest:tt)*) => {
        fn $name<$($gen)*>(self, $($args)*) -> Result<Self::Ok, Error> {
            Err(Error::msg("property string serializer used with a non-object/array type"))
        }

        not_an_object! { $($rest)* }
    };
}

macro_rules! same_impl {
    (as impl<T: fmt::Write> _ for $struct:ident<T> { $($code:tt)* }) => {};
    (
        ser::$trait:ident
        $(ser::$more_traits:ident)*
        as impl<T: fmt::Write> _ for $struct:ident<T> { $($code:tt)* }
    ) => {
        impl<T: fmt::Write> ser::$trait for $struct<T> { $($code)* }
        same_impl! {
            $(ser::$more_traits)*
            as impl<T: fmt::Write> _ for $struct<T> { $($code)* }
        }
    }
}

impl<T: fmt::Write> Serializer for PropertyStringSerializer<T> {
    type Ok = T;
    type Error = Error;

    type SerializeSeq = SerializeSeq<T>;
    type SerializeTuple = SerializeSeq<T>;
    type SerializeTupleStruct = SerializeSeq<T>;
    type SerializeTupleVariant = SerializeSeq<T>;
    type SerializeMap = SerializeStruct<T>;
    type SerializeStruct = SerializeStruct<T>;
    type SerializeStructVariant = SerializeStruct<T>;

    fn is_human_readable(&self) -> bool {
        true
    }

    not_an_object! {
        serialize_bool(bool)
        serialize_i8(i8)
        serialize_i16(i16)
        serialize_i32(i32)
        serialize_i64(i64)
        serialize_u8(u8)
        serialize_u16(u16)
        serialize_u32(u32)
        serialize_u64(u64)
        serialize_f32(f32)
        serialize_f64(f64)
        serialize_char(char)
        serialize_str(&str)
        serialize_bytes(&[u8])
        serialize_none()
        serialize_some<(V: Serialize + ?Sized)>(_value: &V)
        serialize_unit()
        serialize_unit_struct(&'static str)
        serialize_unit_variant(_name: &'static str, _index: u32, _var: &'static str)
    }

    fn serialize_newtype_struct<V>(self, _name: &'static str, value: &V) -> Result<T, Error>
    where
        V: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<V>(
        self,
        name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &V,
    ) -> Result<T, Error>
    where
        V: Serialize + ?Sized,
    {
        Err(Error::msg(format!(
            "cannot serialize enum {name:?} with newtype variants"
        )))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        self.do_seq()
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Error> {
        self.do_seq()
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        self.do_seq()
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        self.do_seq()
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<SerializeStruct<T>, Error> {
        self.do_object()
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<SerializeStruct<T>, Error> {
        self.do_object()
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Error> {
        self.do_object()
    }
}

pub struct SerializeStruct<T> {
    inner: Option<T>,
    comma: bool,
    schema: Option<&'static dyn ObjectSchemaType>,
    /// Schema and, if the value is a type property, its name.
    value_schema: Option<(&'static Schema, Option<&'static str>)>,
    /// This holds both the list which properties *are* type properties (the value is `None`), and
    /// which have already been seen (the value is `Some(...)`).
    type_properties: HashMap<&'static str, Option<String>>,
}

impl<T: fmt::Write> SerializeStruct<T> {
    fn try_new(inner: T, schema: Option<&'static Schema>) -> Result<Self, Error> {
        let mut this = Self {
            schema: match schema {
                None => None,
                Some(schema) => match schema.any_object() {
                    Some(schema) => Some(schema),
                    None => {
                        return Err(Error::msg(
                            "property string serializer used for object with non-object schema",
                        ));
                    }
                },
            },
            inner: Some(inner),
            comma: false,
            value_schema: None,
            type_properties: HashMap::new(),
        };

        if let Some(schema) = schema {
            this.collect_type_properties(schema);
        }

        Ok(this)
    }

    fn collect_type_properties(&mut self, schema: &'static Schema) {
        match schema {
            Schema::OneOf(schema) => {
                self.type_properties.insert(schema.type_property(), None);
                for (_variant, schema) in schema.list {
                    self.collect_type_properties(schema);
                }
            }
            Schema::AllOf(schema) => {
                for schema in schema.list {
                    self.collect_type_properties(schema);
                }
            }
            _ => (),
        }
    }

    fn finish(mut self) -> Result<T, Error> {
        Ok(self.inner.take().unwrap())
    }

    fn do_key<K>(&mut self, key: &K) -> Result<(), Error>
    where
        K: Serialize + ?Sized,
    {
        // While `do_key()` should not be called twice in a row (as per serde documentation), reset
        // the value schema so that bad callers don't get unexpected value schemas:
        self.value_schema = None;

        let key = key.serialize(ElementSerializer::new(String::new(), None))?;

        let inner = self.inner.as_mut().unwrap();
        if mem::replace(&mut self.comma, true) {
            inner.write_char(',')?;
        }

        if let Some(schema) = self.schema {
            let type_property_name = self
                .type_properties
                .get_key_value(&*key) // this way we can turn the key into a `&'static str`
                .map(|(key, _)| *key);

            let value_schema = schema
                .lookup_schema(&key, &|key: &str, func: &mut dyn FnMut(Option<&str>)| {
                    func(
                        self.type_properties
                            .get(key)
                            .and_then(|value| value.as_deref()),
                    )
                })
                .map(|(_optional, schema)| schema);

            if let Some(value_schema) = value_schema {
                self.value_schema = Some((value_schema, type_property_name));
            } else if !schema.additional_properties() {
                // If the key exists in a currently inactive oneOf variant, provide a more useful
                // error message:
                if schema.lookup_all(&key).next().is_some() {
                    return Err(Error::msg(format!(
                        "key {key:?} is not part of the active oneOf variant (it exists in a \
                        different one)"
                    )));
                }
                return Err(Error::msg(format!(
                    "key {key:?} \
                    is not part of the schema and it does not allow additional properties"
                )));
            }
            if schema.default_key() == Some(&key[..]) {
                return Ok(());
            }
        }

        inner.write_str(&key)?;
        inner.write_char('=')?;
        Ok(())
    }

    fn do_value<V>(&mut self, value: &V) -> Result<(), Error>
    where
        V: Serialize + ?Sized,
    {
        let (value_schema, type_property_name) = match self.value_schema.take() {
            None => (None, None),
            Some((schema, type_property_name)) => (Some(schema), type_property_name),
        };

        match type_property_name {
            Some(property_name) => {
                let value = value.serialize(ElementSerializer::new(String::new(), value_schema))?;
                self.inner.as_mut().unwrap().write_str(&value)?;
                // Lookups need the type as parsed, not its quoted property-string form.
                let value = if value.starts_with('"') {
                    crate::property_string::parse_quoted_string(&mut value.as_str())?.into_owned()
                } else {
                    value
                };
                self.type_properties.insert(property_name, Some(value));
            }
            None => {
                let mut inner = self.inner.take().unwrap();
                inner = value.serialize(ElementSerializer::new(inner, value_schema))?;
                self.inner = Some(inner);
            }
        }
        Ok(())
    }
}

same_impl! {
    ser::SerializeStruct
    ser::SerializeStructVariant
    as impl<T: fmt::Write> _ for SerializeStruct<T> {
        type Ok = T;
        type Error = Error;

        fn serialize_field<V>(&mut self, key: &'static str, value: &V) -> Result<(), Self::Error>
        where
            V: Serialize + ?Sized,
        {
            self.do_key(key)?;
            self.do_value(value)
        }

        fn end(self) -> Result<Self::Ok, Self::Error> {
            self.finish()
        }
    }
}

impl<T: fmt::Write> ser::SerializeMap for SerializeStruct<T> {
    type Ok = T;
    type Error = Error;

    fn serialize_key<K>(&mut self, key: &K) -> Result<(), Self::Error>
    where
        K: Serialize + ?Sized,
    {
        self.do_key(key)
    }

    fn serialize_value<V>(&mut self, value: &V) -> Result<(), Self::Error>
    where
        V: Serialize + ?Sized,
    {
        self.do_value(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

pub struct SerializeSeq<T: fmt::Write> {
    inner: Option<T>,
    comma: bool,
    schema: Option<&'static ArraySchema>,
}

impl<T: fmt::Write> SerializeSeq<T> {
    fn new(inner: T, schema: Option<&'static ArraySchema>) -> Self {
        Self {
            inner: Some(inner),
            comma: false,
            schema,
        }
    }

    fn element<V>(&mut self, value: &V) -> Result<(), Error>
    where
        V: Serialize + ?Sized,
    {
        let mut inner = self.inner.take().unwrap();
        if mem::replace(&mut self.comma, true) {
            inner.write_char(',')?;
        }

        inner = value.serialize(ElementSerializer::new(inner, self.schema.map(|s| s.items)))?;
        self.inner = Some(inner);
        Ok(())
    }

    fn finish(mut self) -> Result<T, Error> {
        Ok(self.inner.take().unwrap())
    }
}

same_impl! {
    ser::SerializeSeq
    ser::SerializeTuple
    as impl<T: fmt::Write> _ for SerializeSeq<T> {
        type Ok = T;
        type Error = Error;

        fn serialize_element<V>(&mut self, value: &V) -> Result<(), Error>
        where
            V: Serialize + ?Sized,
        {
            self.element(value)
        }

        fn end(self) -> Result<T, Error> {
            self.finish()
        }
    }
}

same_impl! {
    ser::SerializeTupleStruct
    ser::SerializeTupleVariant
    as impl<T: fmt::Write> _ for SerializeSeq<T> {
        type Ok = T;
        type Error = Error;

        fn serialize_field<V>(&mut self, value: &V) -> Result<(), Error>
        where
            V: Serialize + ?Sized,
        {
            self.element(value)
        }

        fn end(self) -> Result<T, Error> {
            self.finish()
        }
    }
}

pub struct ElementSerializer<T> {
    inner: T,
    schema: Option<&'static Schema>,
}

impl<T> ElementSerializer<T> {
    fn new(inner: T, schema: Option<&'static Schema>) -> Self {
        Self { inner, schema }
    }
}

impl<T: fmt::Write> ElementSerializer<T> {
    fn serialize_with_display<V: fmt::Display>(mut self, v: V) -> Result<T, Error> {
        write!(self.inner, "{v}")
            .map_err(|err| Error::msg(format!("failed to write string: {err}")))?;
        Ok(self.inner)
    }

    fn do_seq(self) -> Result<ElementSerializeSeq<T>, Error> {
        let schema = match self.schema {
            Some(schema) => Some(schema.array().ok_or_else(|| {
                Error::msg("property string serializer used for array with non-array schema")
            })?),
            None => None,
        };
        Ok(ElementSerializeSeq::new(self.inner, schema))
    }

    fn do_object(self) -> Result<ElementSerializeStruct<T>, Error> {
        ElementSerializeStruct::try_new(self.inner, self.schema)
    }
}

macro_rules! forward_to_display {
    () => {};
    ($name:ident($ty:ty) $($rest:tt)*) => {
        fn $name(self, v: $ty) -> Result<Self::Ok, Error> {
            self.serialize_with_display(v)
        }

        forward_to_display! { $($rest)* }
    };
}

impl<T: fmt::Write> Serializer for ElementSerializer<T> {
    type Ok = T;
    type Error = Error;

    type SerializeSeq = ElementSerializeSeq<T>;
    type SerializeTuple = ElementSerializeSeq<T>;
    type SerializeTupleStruct = ElementSerializeSeq<T>;
    type SerializeTupleVariant = ElementSerializeSeq<T>;
    type SerializeMap = ElementSerializeStruct<T>;
    type SerializeStruct = ElementSerializeStruct<T>;
    type SerializeStructVariant = ElementSerializeStruct<T>;

    fn is_human_readable(&self) -> bool {
        true
    }

    forward_to_display! {
        serialize_bool(bool)
        serialize_i8(i8)
        serialize_i16(i16)
        serialize_i32(i32)
        serialize_i64(i64)
        serialize_u8(u8)
        serialize_u16(u16)
        serialize_u32(u32)
        serialize_u64(u64)
        serialize_f32(f32)
        serialize_f64(f64)
        serialize_char(char)
    }

    fn serialize_str(mut self, v: &str) -> Result<Self::Ok, Error> {
        if v.contains(['"', '\\', '\n']) {
            self.inner.write_char('"')?;
            crate::property_string::quote(v, &mut self.inner)?;
            self.inner.write_char('"')?;
        } else {
            self.inner.write_str(v)?;
        }
        Ok(self.inner)
    }

    fn serialize_bytes(self, _: &[u8]) -> Result<Self::Ok, Error> {
        Err(Error::msg(
            "raw byte value not supported in property string",
        ))
    }

    fn serialize_none(self) -> Result<Self::Ok, Error> {
        Err(Error::msg("tried to serialize 'None' value"))
    }

    fn serialize_some<V: Serialize + ?Sized>(self, v: &V) -> Result<Self::Ok, Error> {
        v.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Error> {
        Err(Error::msg("tried to serialize a unit value"))
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Error> {
        Err(Error::msg(format!(
            "tried to serialize a unit value (struct {name})"
        )))
    }

    fn serialize_unit_variant(
        mut self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Error> {
        self.inner.write_str(variant)?;
        Ok(self.inner)
    }

    fn serialize_newtype_struct<V>(self, _name: &'static str, value: &V) -> Result<T, Error>
    where
        V: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<V>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        value: &V,
    ) -> Result<T, Error>
    where
        V: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        self.do_seq()
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Error> {
        self.do_seq()
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        self.do_seq()
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        self.do_seq()
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Error> {
        self.do_object()
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        self.do_object()
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Error> {
        self.do_object()
    }
}

pub struct ElementSerializeStruct<T> {
    output: T,
    inner: SerializeStruct<String>,
}

impl<T: fmt::Write> ElementSerializeStruct<T> {
    fn try_new(inner: T, schema: Option<&'static Schema>) -> Result<Self, Error> {
        Ok(Self {
            inner: SerializeStruct::try_new(String::new(), schema)?,
            output: inner,
        })
    }

    fn finish(mut self) -> Result<T, Error> {
        let value = self.inner.finish()?;
        self.output.write_char('"')?;
        crate::property_string::quote(&value, &mut self.output)?;
        self.output.write_char('"')?;
        Ok(self.output)
    }
}

same_impl! {
    ser::SerializeStruct
    ser::SerializeStructVariant
    as impl<T: fmt::Write> _ for ElementSerializeStruct<T> {
        type Ok = T;
        type Error = Error;

        fn serialize_field<V>(&mut self, key: &'static str, value: &V) -> Result<(), Self::Error>
        where
            V: Serialize + ?Sized,
        {
            self.inner.do_key(key)?;
            self.inner.do_value(value)
        }

        fn end(self) -> Result<Self::Ok, Self::Error> {
            self.finish()
        }
    }
}

impl<T: fmt::Write> ser::SerializeMap for ElementSerializeStruct<T> {
    type Ok = T;
    type Error = Error;

    fn serialize_key<K>(&mut self, key: &K) -> Result<(), Self::Error>
    where
        K: Serialize + ?Sized,
    {
        self.inner.serialize_key(key)
    }

    fn serialize_value<V>(&mut self, value: &V) -> Result<(), Self::Error>
    where
        V: Serialize + ?Sized,
    {
        self.inner.serialize_value(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

pub struct ElementSerializeSeq<T: fmt::Write> {
    output: T,
    inner: SerializeSeq<String>,
}

impl<T: fmt::Write> ElementSerializeSeq<T> {
    fn new(inner: T, schema: Option<&'static ArraySchema>) -> Self {
        Self {
            output: inner,
            inner: SerializeSeq::new(String::new(), schema),
        }
    }

    fn finish(mut self) -> Result<T, Error> {
        let value = self.inner.finish()?;
        if value.contains([',', ';', ' ', '"', '\\', '\n']) {
            self.output.write_char('"')?;
            crate::property_string::quote(&value, &mut self.output)?;
            self.output.write_char('"')?;
        } else {
            self.output.write_str(&value)?;
        }
        Ok(self.output)
    }
}

same_impl! {
    ser::SerializeSeq
    ser::SerializeTuple
    as impl<T: fmt::Write> _ for ElementSerializeSeq<T> {
        type Ok = T;
        type Error = Error;

        fn serialize_element<V>(&mut self, value: &V) -> Result<(), Error>
        where
            V: Serialize + ?Sized,
        {
            self.inner.serialize_element(value)
        }

        fn end(self) -> Result<T, Error> {
            self.finish()
        }
    }
}

same_impl! {
    ser::SerializeTupleStruct
    ser::SerializeTupleVariant
    as impl<T: fmt::Write> _ for ElementSerializeSeq<T> {
        type Ok = T;
        type Error = Error;

        fn serialize_field<V>(&mut self, value: &V) -> Result<(), Error>
        where
            V: Serialize + ?Sized,
        {
            self.inner.element(value)
        }

        fn end(self) -> Result<T, Error> {
            self.finish()
        }
    }
}
