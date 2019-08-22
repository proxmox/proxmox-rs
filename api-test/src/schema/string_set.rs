//! A "set" of strings, semicolon separated, loaded into a `HashSet`.
//!
//! Used as a proxy type for when a struct should contain a `HashSet<String>` which should be
//! serialized as a single comma separated list.

use std::collections::HashSet;

use failure::{bail, Error};

pub trait ForEachStr {
    fn for_each_str<F>(&self, func: F) -> Result<(), Error>
    where
        F: FnMut(&str) -> Result<(), Error>;
}

impl<S: std::hash::BuildHasher> ForEachStr for HashSet<String, S> {
    fn for_each_str<F>(&self, mut func: F) -> Result<(), Error>
    where
        F: FnMut(&str) -> Result<(), Error>,
    {
        for i in self.iter() {
            func(i.as_str())?;
        }
        Ok(())
    }
}

pub fn serialize<S, T: ForEachStr>(value: &T, ser: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let mut data = String::new();
    value
        .for_each_str(|s| {
            if s.contains(';') {
                bail!(
                    "cannot include value \"{}\" in a semicolon separated list",
                    s
                );
            }

            if !data.is_empty() {
                data.push_str(";");
            }
            data.push_str(s);
            Ok(())
        })
        .map_err(serde::ser::Error::custom)?;
    ser.serialize_str(&data)
}

// maybe a custom visitor can also decode arrays by implementing visit_seq?
struct StringSetVisitor;

impl<'de> serde::de::Visitor<'de> for StringSetVisitor {
    type Value = HashSet<String>;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "a string containing semicolon separated elements, or an array of strings"
        )
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(v.split(';').map(|i| i.trim().to_string()).collect())
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut out = seq
            .size_hint()
            .map_or_else(HashSet::new, HashSet::with_capacity);
        loop {
            match seq.next_element::<String>()? {
                Some(el) => out.insert(el),
                None => break,
            };
        }
        Ok(out)
    }
}

pub fn deserialize<'de, D>(de: D) -> Result<HashSet<String>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    de.deserialize_any(StringSetVisitor)
}

pub mod optional {
    use std::collections::HashSet;

    pub fn serialize<S, T: super::ForEachStr>(value: &Option<T>, ser: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match value {
            Some(value) => super::serialize(value, ser),
            None => ser.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Option<HashSet<String>>, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        super::deserialize(de).map(Some)
    }
}
