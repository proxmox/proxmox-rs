//! Serialization/deserialization for memory values with specific units.

use std::marker::PhantomData;

use super::types::Memory;

pub trait Unit {
    const FACTOR: u64;
    const NAME: &'static str;
}

pub struct B;
impl Unit for B {
    const FACTOR: u64 = 1;
    const NAME: &'static str = "bytes";
}

pub struct Kb;
impl Unit for Kb {
    const FACTOR: u64 = 1024;
    const NAME: &'static str = "kilobytes";
}

pub struct Mb;
impl Unit for Mb {
    const FACTOR: u64 = 1024 * 1024;
    const NAME: &'static str = "megabytes";
}

pub struct Gb;
impl Unit for Gb {
    const FACTOR: u64 = 1024 * 1024 * 1024;
    const NAME: &'static str = "gigabytes";
}

struct MemoryVisitor<U: Unit>(PhantomData<U>);
impl<'de, U: Unit> serde::de::Visitor<'de> for MemoryVisitor<U> {
    type Value = Memory;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "amount of memory in {}", U::NAME)
    }

    fn visit_u8<E: serde::de::Error>(self, v: u8) -> Result<Self::Value, E> {
        Ok(Memory::from_bytes(v as u64 * U::FACTOR))
    }
    fn visit_u16<E: serde::de::Error>(self, v: u16) -> Result<Self::Value, E> {
        Ok(Memory::from_bytes(v as u64 * U::FACTOR))
    }
    fn visit_u32<E: serde::de::Error>(self, v: u32) -> Result<Self::Value, E> {
        Ok(Memory::from_bytes(v as u64 * U::FACTOR))
    }
    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
        Ok(Memory::from_bytes(v as u64 * U::FACTOR))
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        match v.parse::<u64>() {
            Ok(v) => Ok(Memory::from_bytes(v * U::FACTOR)),
            Err(_) => v.parse().map_err(serde::de::Error::custom),
        }
    }
}

pub struct Parser<U: Unit>(PhantomData<U>);

impl<U: Unit> Parser<U> {
    pub fn serialize<S>(value: &Memory, ser: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if (value.as_bytes() % U::FACTOR) == 0 {
            ser.serialize_u64(value.as_bytes() / U::FACTOR)
        } else {
            ser.serialize_str(&value.to_string())
        }
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Memory, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        de.deserialize_any(MemoryVisitor::<U>(PhantomData))
    }
}

pub mod optional {
    use std::marker::PhantomData;

    use super::Unit;
    use crate::schema::types::Memory;

    pub struct Parser<U: Unit>(PhantomData<U>);

    impl<U: Unit> Parser<U> {
        pub fn serialize<S>(value: &Option<Memory>, ser: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            super::Parser::<U>::serialize::<S>(&value.unwrap(), ser)
        }

        pub fn deserialize<'de, D>(de: D) -> Result<Option<Memory>, D::Error>
        where
            D: serde::de::Deserializer<'de>,
        {
            super::Parser::<U>::deserialize::<'de, D>(de).map(Some)
        }
    }
}
