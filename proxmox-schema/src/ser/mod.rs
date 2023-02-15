//! Property string serialization.

use std::fmt;
use std::mem;

use serde::ser::{self, Serialize, Serializer};

use crate::de::Error;

impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::msg(msg.to_string())
    }
}

pub struct PropertyStringSerializer<T> {
    inner: T,
}

impl<T> PropertyStringSerializer<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

macro_rules! not_an_object {
    () => {};
    ($name:ident($ty:ty) $($rest:tt)*) => {
        fn $name(self, _v: $ty) -> Result<Self::Ok, Error> {
            Err(Error::msg("property string serializer used with a non-object type"))
        }

        not_an_object! { $($rest)* }
    };
    ($name:ident($($args:tt)*) $($rest:tt)*) => {
        fn $name(self, $($args)*) -> Result<Self::Ok, Error> {
            Err(Error::msg("property string serializer used with a non-object type"))
        }

        not_an_object! { $($rest)* }
    };
    ($name:ident<($($gen:tt)*)>($($args:tt)*) $($rest:tt)*) => {
        fn $name<$($gen)*>(self, $($args)*) -> Result<Self::Ok, Error> {
            Err(Error::msg("property string serializer used with a non-object type"))
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
        Ok(SerializeSeq::new(self.inner))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Error> {
        Ok(SerializeSeq::new(self.inner))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        Ok(SerializeSeq::new(self.inner))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Ok(SerializeSeq::new(self.inner))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<SerializeStruct<T>, Error> {
        Ok(SerializeStruct::new(self.inner))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<SerializeStruct<T>, Error> {
        Ok(SerializeStruct::new(self.inner))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Ok(SerializeStruct::new(self.inner))
    }
}

pub struct SerializeStruct<T> {
    inner: Option<T>,
    comma: bool,
}

impl<T: fmt::Write> SerializeStruct<T> {
    fn new(inner: T) -> Self {
        Self {
            inner: Some(inner),
            comma: false,
        }
    }

    fn field<V>(&mut self, key: &'static str, value: &V) -> Result<(), Error>
    where
        V: Serialize + ?Sized,
    {
        let mut inner = self.inner.take().unwrap();

        if mem::replace(&mut self.comma, true) {
            inner.write_char(',')?;
        }
        write!(inner, "{key}=")?;
        self.inner = Some(value.serialize(ElementSerializer::new(inner))?);
        Ok(())
    }

    fn finish(mut self) -> Result<T, Error> {
        Ok(self.inner.take().unwrap())
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
            self.field(key, value)
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
        let mut inner = self.inner.take().unwrap();
        if mem::replace(&mut self.comma, true) {
            inner.write_char(',')?;
        }
        inner = key.serialize(ElementSerializer::new(inner))?;
        inner.write_char('=')?;
        self.inner = Some(inner);
        Ok(())
    }

    fn serialize_value<V>(&mut self, value: &V) -> Result<(), Self::Error>
    where
        V: Serialize + ?Sized,
    {
        let mut inner = self.inner.take().unwrap();
        inner = value.serialize(ElementSerializer::new(inner))?;
        self.inner = Some(inner);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

pub struct SerializeSeq<T: fmt::Write> {
    inner: Option<T>,
    comma: bool,
}

impl<T: fmt::Write> SerializeSeq<T> {
    fn new(inner: T) -> Self {
        Self {
            inner: Some(inner),
            comma: false,
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

        inner = value.serialize(ElementSerializer::new(inner))?;
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
}

impl<T> ElementSerializer<T> {
    fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T: fmt::Write> ElementSerializer<T> {
    fn serialize_with_display<V: fmt::Display>(mut self, v: V) -> Result<T, Error> {
        write!(self.inner, "{v}")
            .map_err(|err| Error::msg(format!("failed to write string: {err}")))?;
        Ok(self.inner)
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
        if v.contains(&['"', '\\', '\n']) {
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
        self,
        name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Error> {
        Err(Error::msg(format!(
            "tried to serialize a unit variant ({name}::{variant})"
        )))
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
        Ok(ElementSerializeSeq::new(self.inner))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Error> {
        Ok(ElementSerializeSeq::new(self.inner))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        Ok(ElementSerializeSeq::new(self.inner))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Ok(ElementSerializeSeq::new(self.inner))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Error> {
        Ok(ElementSerializeStruct::new(self.inner))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        Ok(ElementSerializeStruct::new(self.inner))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Ok(ElementSerializeStruct::new(self.inner))
    }
}

pub struct ElementSerializeStruct<T> {
    output: T,
    inner: SerializeStruct<String>,
}

impl<T: fmt::Write> ElementSerializeStruct<T> {
    fn new(inner: T) -> Self {
        Self {
            output: inner,
            inner: SerializeStruct::new(String::new()),
        }
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
            self.inner.field(key, value)
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
    fn new(inner: T) -> Self {
        Self {
            output: inner,
            inner: SerializeSeq::new(String::new()),
        }
    }

    fn finish(mut self) -> Result<T, Error> {
        let value = self.inner.finish()?;
        if value.contains(&[',', ';', ' ', '"', '\\', '\n']) {
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
