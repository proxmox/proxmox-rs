//! Comma separated string list.
//!
//! Used as a proxy type for when a struct should contain a `Vec<String>` which should be
//! serialized as a single comma separated list.

use failure::{bail, Error};

pub trait ForEachStr {
    fn for_each_str<F>(&self, func: F) -> Result<(), Error>
    where
        F: FnMut(&str) -> Result<(), Error>;
}

impl ForEachStr for Vec<String> {
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
            if s.contains(',') {
                bail!("cannot include value \"{}\" in a comma separated list", s);
            }

            if !data.is_empty() {
                data.push_str(", ");
            }
            data.push_str(s);
            Ok(())
        })
        .map_err(serde::ser::Error::custom)?;
    ser.serialize_str(&data)
}

// maybe a custom visitor can also decode arrays by implementing visit_seq?
struct StringListVisitor;

impl<'de> serde::de::Visitor<'de> for StringListVisitor {
    type Value = Vec<String>;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "a comma separated list as a string, or an array of strings"
        )
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(v.split(',').map(|i| i.trim().to_string()).collect())
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut out = seq.size_hint().map_or_else(Vec::new, Vec::with_capacity);
        while let Some(el) = seq.next_element::<String>()? {
            out.push(el);
        }
        Ok(out)
    }
}

pub fn deserialize<'de, D>(de: D) -> Result<Vec<String>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    de.deserialize_any(StringListVisitor)
}

pub mod optional {
    pub fn serialize<S, T: super::ForEachStr>(value: &Option<T>, ser: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match value {
            Some(value) => super::serialize(value, ser),
            None => ser.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Option<Vec<String>>, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        super::deserialize(de).map(Some)
    }
}
