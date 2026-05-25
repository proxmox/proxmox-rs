//! Implements a serde serializer for the INI file format.
//!
//! Nested structs/maps are supported and use the widely used variant of using dots as hierarchy
//! separators.
//!
//! Newtype variants, tuple variants, struct variants and raw bytes are not supported.

#![forbid(unsafe_code, missing_docs)]

use std::{
    collections::BTreeMap,
    fmt::{self, Display, Write},
    io,
};

use serde::ser::{self, Impossible, Serialize};

#[derive(Debug, PartialEq)]
/// Errors that can occur during INI serialization.
pub enum Error {
    /// Some error that occurred elsewhere.
    Generic(String),
    /// Error during I/O.
    Io(String),
    /// Encountered an unsupported data type during serialization.
    UnsupportedType(&'static str),
    /// A key was expected at this point during serialization, but a value was received.
    ExpectedKey,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Generic(s) => write!(f, "{s}"),
            Error::Io(s) => write!(f, "{s}"),
            Error::UnsupportedType(s) => write!(f, "unsupported data type: {s}"),
            Error::ExpectedKey => write!(f, "expected key"),
        }
    }
}

impl std::error::Error for Error {}

impl ser::Error for Error {
    fn custom<T: fmt::Display>(err: T) -> Self {
        Error::Generic(err.to_string())
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<fmt::Error> for Error {
    fn from(err: fmt::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(err: std::string::FromUtf8Error) -> Self {
        Self::Generic(err.to_string())
    }
}

/// Return type used throughout the serializer.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq)]
/// Type of serialized value.
enum SerializedType {
    /// Last serialized value was a key-value pair, ie. `key = value`.
    Simple,
    /// Last serialized was a section.
    Section,
}

/// Implements a serde serializer for transforming Rust values into the INI
/// format.
#[derive(Debug)]
struct IniSerializer {
    /// Last key observed during serialization
    last_key: Option<String>,
    /// Already serialized key-value pairs on this level of the serialization tree
    buf: String,
    /// Nested sections under this part of the tree. Multiple sections with the
    /// same name are allowed.
    sections: BTreeMap<String, Vec<String>>,
}

impl IniSerializer {
    /// Creates a new INI serializer.
    fn new() -> Self {
        IniSerializer {
            last_key: None,
            buf: String::new(),
            sections: BTreeMap::new(),
        }
    }

    /// Write out the serialized INI to a target implementing [`io::Write`].
    fn write<W: io::Write>(self, mut w: W) -> Result<()> {
        w.write_all(self.buf.as_bytes())?;
        if !self.buf.is_empty() && !self.sections.is_empty() {
            w.write_all(b"\n")?;
        }

        for (index, (name, values)) in self.sections.iter().enumerate() {
            for (nested_idx, section) in values.iter().enumerate() {
                write!(w, "[{name}]\n{section}")?;

                if nested_idx < values.len() - 1 {
                    w.write_all(b"\n")?;
                }
            }

            if index < self.sections.len() - 1 {
                w.write_all(b"\n")?;
            }
        }

        Ok(())
    }

    /// Serializes a value using its [`Display`] implementation.
    /// If a key is known for this value, it's prepended to the output and forgotten.
    fn serialize_as_display<T: Display>(&mut self, v: T) -> Result<SerializedType> {
        if let Some(key) = self.last_key.take() {
            writeln!(self.buf, "{key} = {v}")?;
        } else {
            self.buf += &v.to_string();
        }
        Ok(SerializedType::Simple)
    }
}

/// Serialize the given data structure as INI into an I/O stream.
pub fn to_writer<W, T>(writer: W, value: &T) -> Result<()>
where
    W: io::Write,
    T: ?Sized + Serialize,
{
    let mut ser = IniSerializer::new();
    value.serialize(&mut ser)?;
    ser.write(writer)
}

/// Serialize the given data structure as INI into a string.
pub fn to_string<T>(value: &T) -> Result<String>
where
    T: ?Sized + Serialize,
{
    let mut buf = Vec::new();
    to_writer(&mut buf, value)?;

    Ok(String::from_utf8(buf)?)
}

macro_rules! forward_to_display {
    ($name:ident($ty:ty), $($rest:tt)* ) => {
        fn $name(self, v: $ty) -> Result<Self::Ok> {
            self.serialize_as_display(&v)
        }

        forward_to_display! { $($rest)* }
    };
    () => {};
}

impl<'a> ser::Serializer for &'a mut IniSerializer {
    type Ok = SerializedType;
    type Error = Error;

    type SerializeSeq = IniSeqSerializer<'a>;
    type SerializeTuple = IniSeqSerializer<'a>;
    type SerializeTupleStruct = IniSeqSerializer<'a>;
    type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
    type SerializeMap = IniMapSerializer<'a>;
    type SerializeStruct = IniMapSerializer<'a>;
    type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

    forward_to_display! {
        serialize_bool(bool),
        serialize_i8(i8),
        serialize_i16(i16),
        serialize_i32(i32),
        serialize_i64(i64),
        serialize_u8(u8),
        serialize_u16(u16),
        serialize_u32(u32),
        serialize_u64(u64),
        serialize_f32(f32),
        serialize_f64(f64),
        serialize_char(char),
        serialize_str(&str),
    }

    fn serialize_bytes(self, _: &[u8]) -> Result<Self::Ok> {
        Err(Error::UnsupportedType("raw bytes"))
    }

    fn serialize_none(self) -> Result<Self::Ok> {
        self.last_key = None;
        Ok(Self::Ok::Simple)
    }

    fn serialize_some<T>(self, v: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        v.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok> {
        self.serialize_none()
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok> {
        self.serialize_none()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::UnsupportedType("enum newtype variant"))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Ok(IniSeqSerializer::new(self))
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(Error::UnsupportedType("enum tuple variant"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Ok(IniMapSerializer {
            ser: self,
            last_key: None,
        })
    }

    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(Error::UnsupportedType("enum struct variant"))
    }
}

struct IniMapSerializer<'a> {
    /// Root serializer.
    ser: &'a mut IniSerializer,
    /// Last serialized key observed at this level.
    last_key: Option<String>,
}

impl<'a> ser::SerializeMap for IniMapSerializer<'a> {
    type Ok = SerializedType;
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let mut s = String::new();
        key.serialize(IniKeySerializer::new(&mut s))?;

        self.last_key = Some(s);
        self.ser.last_key = self.last_key.clone();

        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let mut serializer = IniSerializer::new();
        serializer.last_key = self.ser.last_key.clone();

        let key = self.last_key.clone().ok_or(Error::ExpectedKey)?;

        match value.serialize(&mut serializer)? {
            SerializedType::Simple => {
                // Value serialized as a primitive type, we can just write that out

                self.ser.buf += &serializer.buf;
            }
            SerializedType::Section => {
                if !serializer.buf.is_empty() {
                    // First, add all top-level entries from the map into a new section,
                    // in case we serialized a map
                    self.ser
                        .sections
                        .entry(key.clone())
                        .or_default()
                        .push(serializer.buf);
                } else if let Some(mut values) = serializer.sections.remove(&key) {
                    // Otherwise we serialized a sequence of maps, append all of them under the current
                    // name
                    self.ser
                        .sections
                        .entry(key.clone())
                        .or_default()
                        .append(&mut values);
                }

                // .. and finally, append all other nested sections
                for (name, mut values) in serializer.sections {
                    self.ser
                        .sections
                        .entry(format!("{key}.{name}"))
                        .or_default()
                        .append(&mut values);
                }
            }
        }

        Ok(())
    }

    fn end(self) -> Result<Self::Ok> {
        Ok(Self::Ok::Section)
    }
}

impl<'a> ser::SerializeStruct for IniMapSerializer<'a> {
    type Ok = SerializedType;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        ser::SerializeMap::serialize_key(self, key)?;
        ser::SerializeMap::serialize_value(self, value)
    }

    fn end(self) -> Result<Self::Ok> {
        ser::SerializeMap::end(self)
    }
}

struct IniSeqSerializer<'a> {
    /// Root serializer.
    ser: &'a mut IniSerializer,
    /// Whether at least one element has been serialized yet.
    first: bool,
    /// Whether we saw at least one section in the past.
    has_sections: bool,
}

impl<'a> IniSeqSerializer<'a> {
    pub fn new(ser: &'a mut IniSerializer) -> Self {
        Self {
            ser,
            first: true,
            has_sections: false,
        }
    }
}

impl ser::SerializeSeq for IniSeqSerializer<'_> {
    type Ok = SerializedType;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        // As we (at least, for now) don't support enum newtype variants, types which serialize to
        // either a primitive type or a section type cannot be represested.

        let mut serializer = IniSerializer::new();
        let key = self.ser.last_key.clone().ok_or(Error::ExpectedKey)?;

        match value.serialize(&mut serializer)? {
            SerializedType::Simple => {
                // Value serialized as a primitive type, so write it out
                if !self.first {
                    write!(self.ser.buf, ", {}", serializer.buf)?;
                } else {
                    write!(self.ser.buf, "{key} = {}", serializer.buf)?;
                    self.first = false;
                }
                Ok(())
            }
            SerializedType::Section => {
                self.has_sections = true;

                self.ser
                    .sections
                    .entry(key.clone())
                    .or_default()
                    .push(serializer.buf);

                for (name, mut values) in serializer.sections {
                    self.ser
                        .sections
                        .entry(format!("{key}.{name}"))
                        .or_default()
                        .append(&mut values);
                }

                Ok(())
            }
        }
    }

    fn end(self) -> Result<Self::Ok> {
        if self.has_sections {
            Ok(Self::Ok::Section)
        } else {
            if !self.first {
                self.ser.buf.push('\n');
            }
            Ok(Self::Ok::Simple)
        }
    }
}

impl ser::SerializeTuple for IniSeqSerializer<'_> {
    type Ok = SerializedType;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for IniSeqSerializer<'_> {
    type Ok = SerializedType;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok> {
        ser::SerializeSeq::end(self)
    }
}

/// Slimmed down serializer which just supports serializing single values to the given writer and
/// no compound values.
///
/// Used for serializing keys to their string representation.
struct IniKeySerializer<'a, W: fmt::Write> {
    /// Target to write any serialized value to.
    writer: &'a mut W,
}

impl<'a, W: fmt::Write> IniKeySerializer<'a, W> {
    fn new(writer: &'a mut W) -> Self {
        Self { writer }
    }
}

macro_rules! forward_to_writer_as_str {
    ($name:ident($ty:ty), $($rest:tt)* ) => {
        fn $name(self, v: $ty) -> Result<Self::Ok> {
            self.writer.write_str(&v.to_string())?;
            Ok(())
        }

        forward_to_writer_as_str! { $($rest)* }
    };
    () => {};
}

impl<'a, W: fmt::Write> ser::Serializer for IniKeySerializer<'a, W> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
    type SerializeMap = Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

    forward_to_writer_as_str! {
        serialize_bool(bool),
        serialize_i8(i8),
        serialize_i16(i16),
        serialize_i32(i32),
        serialize_i64(i64),
        serialize_u8(u8),
        serialize_u16(u16),
        serialize_u32(u32),
        serialize_u64(u64),
        serialize_f32(f32),
        serialize_f64(f64),
        serialize_char(char),
        serialize_str(&str),
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok> {
        Err(Error::UnsupportedType("raw bytes"))
    }

    fn serialize_none(self) -> Result<Self::Ok> {
        Ok(())
    }

    fn serialize_some<T>(self, v: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        v.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok> {
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok> {
        Ok(())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::UnsupportedType("nested newtype variant"))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(Error::UnsupportedType("nested sequence"))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Err(Error::UnsupportedType("nested tuple"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(Error::UnsupportedType("nested tuple struct"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(Error::UnsupportedType("nested tuple variant"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(Error::UnsupportedType("nested maps"))
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Err(Error::UnsupportedType("nested structs"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(Error::UnsupportedType("nested struct variant"))
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, ffi::CString, marker::PhantomData};

    use super::{Error, to_string};
    use serde::Serialize;

    #[test]
    fn all_supported_types() {
        #[derive(Serialize)]
        struct NestedStruct {
            s: &'static str,
            x: f64,
            l: Vec<&'static str>,
            c: char,
        }

        #[derive(Serialize)]
        enum Enum {
            A,
        }

        #[derive(Serialize)]
        struct NewtypeStruct(u8);

        #[derive(Serialize)]
        struct TopLevel {
            a: u32,
            nested: NestedStruct,
            none: Option<i32>,
            some: Option<i32>,
            bytes: [u8; 3],
            unit: (),
            unit_struct: PhantomData<u32>,
            unit_variant: Enum,
            newtype_struct: NewtypeStruct,
            list: Vec<u32>,
            empty_list: Vec<u32>,
            one_item_list: Vec<u32>,
            tuple: (u32, &'static str),
        }

        let serialized = to_string(&TopLevel {
            a: 1,
            nested: NestedStruct {
                s: "foo",
                x: 123.4567,
                l: vec!["a", "b", "c"],
                c: 'Y',
            },
            none: None,
            some: Some(42),
            bytes: [1, 2, 3],
            unit: (),
            unit_struct: PhantomData,
            unit_variant: Enum::A,
            newtype_struct: NewtypeStruct(42),
            list: vec![100, 200, 300],
            empty_list: Vec::new(),
            one_item_list: vec![42],
            tuple: (123, "bar"),
        })
        .unwrap();

        pretty_assertions::assert_eq!(
            "a = 1
some = 42
bytes = 1, 2, 3
unit_variant = A
newtype_struct = 42
list = 100, 200, 300
one_item_list = 42
tuple = 123, bar

[nested]
s = foo
x = 123.4567
l = a, b, c
c = Y
",
            serialized,
        );
    }

    #[test]
    fn two_levels_nested() {
        #[derive(Serialize)]
        struct SecondLevel {
            x: u32,
        }

        #[derive(Serialize)]
        struct FirstLevel {
            b: f32,
            second_level: SecondLevel,
        }

        #[derive(Serialize)]
        struct NestedStruct {
            s: &'static str,
        }

        #[derive(Serialize)]
        struct TopLevel {
            a: u32,
            nested: NestedStruct,
            first_level: FirstLevel,
        }

        let serialized = to_string(&TopLevel {
            a: 1,
            nested: NestedStruct { s: "foo" },
            first_level: FirstLevel {
                b: 12.3,
                second_level: SecondLevel { x: 100 },
            },
        })
        .unwrap();

        pretty_assertions::assert_eq!(
            "a = 1

[first_level]
b = 12.3

[first_level.second_level]
x = 100

[nested]
s = foo
",
            serialized,
        );
    }

    #[test]
    fn no_top_level_kvs() {
        #[derive(Serialize)]
        struct NestedStruct {
            s: &'static str,
        }

        #[derive(Serialize)]
        struct TopLevel {
            a: NestedStruct,
            b: NestedStruct,
        }

        let serialized = to_string(&TopLevel {
            a: NestedStruct { s: "foo" },
            b: NestedStruct { s: "bar" },
        })
        .unwrap();

        pretty_assertions::assert_eq!(
            "[a]
s = foo

[b]
s = bar
",
            serialized,
        );
    }

    #[test]
    fn unsupported_datatypes() {
        #[derive(Serialize)]
        enum Enum {
            A(u32),
            B(u32, f32),
            C { a: u8, b: &'static str },
        }

        #[derive(Serialize)]
        struct TopLevel {
            x: Enum,
        }

        #[derive(Serialize)]
        struct RawBytes {
            s: CString,
        }

        assert_eq!(
            Err(Error::UnsupportedType("enum newtype variant")),
            to_string(&TopLevel { x: Enum::A(1) }),
        );

        assert_eq!(
            Err(Error::UnsupportedType("enum tuple variant")),
            to_string(&TopLevel { x: Enum::B(1, 2.) }),
        );

        assert_eq!(
            Err(Error::UnsupportedType("enum struct variant")),
            to_string(&TopLevel {
                x: Enum::C {
                    a: 100,
                    b: "foobar"
                }
            }),
        );

        assert_eq!(
            Err(Error::UnsupportedType("raw bytes")),
            to_string(&RawBytes {
                s: CString::new("baz").unwrap(),
            })
        );
    }

    #[test]
    fn multiple_sections_with_same_name() {
        #[derive(Serialize)]
        struct NestedStruct {
            x: u32,
        }

        #[derive(Serialize)]
        struct TopLevel {
            a: u32,
            nested: Vec<NestedStruct>,
        }

        let serialized = to_string(&TopLevel {
            a: 42,
            nested: vec![
                NestedStruct { x: 1 },
                NestedStruct { x: 2 },
                NestedStruct { x: 3 },
            ],
        })
        .unwrap();

        pretty_assertions::assert_eq!(
            "a = 42

[nested]
x = 1

[nested]
x = 2

[nested]
x = 3
",
            serialized,
        );
    }

    #[test]
    fn unsupported_nested_lists() {
        #[derive(Serialize)]
        struct TopLevel {
            x: Vec<Vec<u32>>,
        }

        assert_eq!(
            Err(Error::ExpectedKey),
            to_string(&TopLevel {
                x: vec![vec![1, 2], vec![3, 4]],
            }),
        );
    }

    #[test]
    fn empty_struct_should_produce_nothing() {
        #[derive(Serialize)]
        struct Empty {}

        #[derive(Serialize)]
        struct TopLevel {
            empty: Empty,
        }

        let serialized = to_string(&TopLevel { empty: Empty {} }).unwrap();
        pretty_assertions::assert_eq!("", serialized);
    }

    #[test]
    fn deeply_nested() {
        #[derive(Serialize)]
        struct ThirdLevel {
            x: u32,
        }

        #[derive(Serialize)]
        struct SecondLevel {
            third: ThirdLevel,
        }

        #[derive(Serialize)]
        struct FirstLevel {
            second: SecondLevel,
        }

        #[derive(Serialize)]
        struct TopLevel {
            first: FirstLevel,
        }

        let serialized = to_string(&TopLevel {
            first: FirstLevel {
                second: SecondLevel {
                    third: ThirdLevel { x: 1 },
                },
            },
        })
        .unwrap();

        pretty_assertions::assert_eq!(
            r#"[first.second.third]
x = 1
"#,
            serialized
        );
    }

    #[test]
    fn ints_as_keys() {
        let mut map = BTreeMap::new();
        map.insert(1u32, "one");
        map.insert(2, "two");

        pretty_assertions::assert_eq!(
            r#"1 = one
2 = two
"#,
            to_string(&map).unwrap()
        );
    }
}
