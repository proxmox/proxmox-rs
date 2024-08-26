//! Support for `enum` typed section configs.

use std::collections::HashMap;

use anyhow::Error;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

use crate::SectionConfig;
use crate::SectionConfigData as RawSectionConfigData;

/// Implement this for an enum to allow it to be used as a section config.
pub trait ApiSectionDataEntry: Sized {
    /// If the serde representation is internally tagged, this should be the name of the type
    /// property.
    const INTERNALLY_TAGGED: Option<&'static str> = None;

    /// If the [`SectionConfig`] returned by the [`section_config()`][seccfg] method includes the
    /// `.with_type_key()` properties correctly, this should be set to `true`, otherwise `false`
    /// (which is the default).
    ///
    /// [seccfg]: Self::section_config()
    const SECION_CONFIG_USES_TYPE_KEY: bool = false;

    /// Get the `SectionConfig` configuration for this enum.
    fn section_config() -> &'static SectionConfig;

    /// Maps an enum value to its type name.
    fn section_type(&self) -> &'static str;

    /// Provided. If necessary, insert the "type" property into an object and then deserialize it.
    fn from_value(ty: String, mut value: Value) -> Result<Self, serde_json::Error>
    where
        Self: serde::de::DeserializeOwned,
    {
        if Self::SECION_CONFIG_USES_TYPE_KEY {
            serde_json::from_value::<Self>(value)
        } else if let Some(tag) = Self::INTERNALLY_TAGGED {
            match &mut value {
                Value::Object(obj) => {
                    obj.insert(tag.to_string(), ty.into());
                    serde_json::from_value::<Self>(value)
                }
                _ => {
                    use serde::ser::Error;
                    Err(serde_json::Error::custom(
                        "cannot add type property to non-object",
                    ))
                }
            }
        } else {
            serde_json::from_value::<Self>(json!({ ty: value }))
        }
    }

    /// Turn this entry into a pair of `(type, value)`.
    /// Works for externally tagged objects and internally tagged objects, provided the
    /// `INTERNALLY_TAGGED` value is set.
    fn into_pair(self) -> Result<(String, Value), serde_json::Error>
    where
        Self: Serialize,
    {
        to_pair(
            serde_json::to_value(self)?,
            Self::INTERNALLY_TAGGED,
            !Self::SECION_CONFIG_USES_TYPE_KEY,
        )
    }

    /// Turn this entry into a pair of `(type, value)`.
    /// Works for externally tagged objects and internally tagged objects, provided the
    /// `INTERNALLY_TAGGED` value is set.
    fn to_pair(&self) -> Result<(String, Value), serde_json::Error>
    where
        Self: Serialize,
    {
        to_pair(
            serde_json::to_value(self)?,
            Self::INTERNALLY_TAGGED,
            !Self::SECION_CONFIG_USES_TYPE_KEY,
        )
    }

    /// Provided. Shortcut for `Self::section_config().parse(filename, data)?.try_into()`.
    fn parse_section_config(filename: &str, data: &str) -> Result<SectionConfigData<Self>, Error>
    where
        Self: serde::de::DeserializeOwned,
    {
        Ok(Self::section_config().parse(filename, data)?.try_into()?)
    }

    /// Provided. Shortcut for `Self::section_config().write(filename, &data.try_into()?)`.
    fn write_section_config(filename: &str, data: &SectionConfigData<Self>) -> Result<String, Error>
    where
        Self: Serialize,
    {
        Self::section_config().write(filename, &data.try_into()?)
    }
}

/// Turn an object into a `(type, value)` pair.
///
/// For internally tagged objects (`tag` is `Some`), the type is *extracted* first. It is then no
/// longer present in the object itself.
///
/// Otherwise, an externally typed object is expected, which means a map with a single entry, with
/// the type being the key.
///
/// Otherwise this will fail.
fn to_pair(
    value: Value,
    tag: Option<&'static str>,
    strip_tag: bool,
) -> Result<(String, Value), serde_json::Error> {
    use serde::ser::Error;

    match (value, tag) {
        (Value::Object(mut obj), Some(tag)) => {
            let id = if strip_tag {
                obj.remove(tag)
                    .ok_or_else(|| Error::custom(format!("tag {tag:?} missing in object")))?
            } else {
                obj.get(tag)
                    .ok_or_else(|| Error::custom(format!("tag {tag:?} missing in object")))?
                    .clone()
            };
            match id {
                Value::String(id) => Ok((id, Value::Object(obj))),
                _ => Err(Error::custom(format!(
                    "tag {tag:?} has invalid value (not a string)"
                ))),
            }
        }
        (Value::Object(obj), None) if obj.len() == 1 => Ok(
            obj.into_iter().next().unwrap(), // unwrap: we checked the length
        ),
        _ => Err(Error::custom("unexpected serialization method")),
    }
}

/// Typed variant of [`SectionConfigData`](crate::SectionConfigData).
/// This dereferences to the section hash for convenience.
#[derive(Debug, Clone)]
pub struct SectionConfigData<T> {
    pub sections: HashMap<String, T>,
    pub order: Vec<String>,
}

impl<T> Default for SectionConfigData<T> {
    fn default() -> Self {
        Self {
            sections: HashMap::new(),
            order: Vec::new(),
        }
    }
}

impl<T: ApiSectionDataEntry + DeserializeOwned> TryFrom<RawSectionConfigData>
    for SectionConfigData<T>
{
    type Error = serde_json::Error;

    fn try_from(data: RawSectionConfigData) -> Result<Self, serde_json::Error> {
        let sections =
            data.sections
                .into_iter()
                .try_fold(HashMap::new(), |mut acc, (id, (ty, value))| {
                    acc.insert(id, T::from_value(ty, value)?);
                    Ok::<_, serde_json::Error>(acc)
                })?;
        Ok(Self {
            sections,
            order: data.order,
        })
    }
}

impl<T> std::ops::Deref for SectionConfigData<T> {
    type Target = HashMap<String, T>;

    fn deref(&self) -> &Self::Target {
        &self.sections
    }
}

impl<T> std::ops::DerefMut for SectionConfigData<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sections
    }
}

impl<T: Serialize + ApiSectionDataEntry> TryFrom<SectionConfigData<T>> for RawSectionConfigData {
    type Error = serde_json::Error;

    fn try_from(data: SectionConfigData<T>) -> Result<Self, serde_json::Error> {
        let sections =
            data.sections
                .into_iter()
                .try_fold(HashMap::new(), |mut acc, (id, value)| {
                    acc.insert(id, value.into_pair()?);
                    Ok::<_, serde_json::Error>(acc)
                })?;

        Ok(Self {
            sections,
            order: data.order,
        })
    }
}

impl<T: Serialize + ApiSectionDataEntry> TryFrom<&SectionConfigData<T>> for RawSectionConfigData {
    type Error = serde_json::Error;

    fn try_from(data: &SectionConfigData<T>) -> Result<Self, serde_json::Error> {
        let sections = data
            .sections
            .iter()
            .try_fold(HashMap::new(), |mut acc, (id, value)| {
                acc.insert(id.clone(), value.to_pair()?);
                Ok::<_, serde_json::Error>(acc)
            })?;

        Ok(Self {
            sections,
            order: data.order.clone(),
        })
    }
}

/// Creates an unordered data set.
impl<T: ApiSectionDataEntry> From<HashMap<String, T>> for SectionConfigData<T> {
    fn from(sections: HashMap<String, T>) -> Self {
        Self {
            sections,
            order: Vec::new(),
        }
    }
}

/// Creates a data set ordered the same way as the iterator.
impl<T: ApiSectionDataEntry> FromIterator<(String, T)> for SectionConfigData<T> {
    fn from_iter<I: IntoIterator<Item = (String, T)>>(iter: I) -> Self {
        let mut sections = HashMap::new();
        let mut order = Vec::new();

        for (key, value) in iter {
            order.push(key.clone());
            sections.insert(key, value);
        }

        Self { sections, order }
    }
}

impl<T> IntoIterator for SectionConfigData<T> {
    type IntoIter = IntoIter<T>;
    type Item = (String, T);

    fn into_iter(self) -> IntoIter<T> {
        IntoIter {
            sections: self.sections,
            order: self.order.into_iter(),
        }
    }
}

/// Iterates over the sections in their original order.
pub struct IntoIter<T> {
    sections: HashMap<String, T>,
    order: std::vec::IntoIter<String>,
}

impl<T> Iterator for IntoIter<T> {
    type Item = (String, T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let id = self.order.next()?;
            if let Some(data) = self.sections.remove(&id) {
                return Some((id, data));
            }
        }
    }
}

impl<'a, T> IntoIterator for &'a SectionConfigData<T> {
    type IntoIter = Iter<'a, T>;
    type Item = (&'a str, &'a T);

    fn into_iter(self) -> Iter<'a, T> {
        Iter {
            sections: &self.sections,
            order: self.order.iter(),
        }
    }
}

/// Iterates over the sections in their original order.
pub struct Iter<'a, T> {
    sections: &'a HashMap<String, T>,
    order: std::slice::Iter<'a, String>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (&'a str, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let id = self.order.next()?;
            if let Some(data) = self.sections.get(id) {
                return Some((id, data));
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::sync::OnceLock;

    use proxmox_schema::{ApiStringFormat, EnumEntry, ObjectSchema, Schema, StringSchema};

    use crate::{SectionConfig, SectionConfigPlugin};

    use super::{ApiSectionDataEntry, SectionConfigData};

    enum Ty {
        A,
        B,
    }

    struct Entry {
        ty: Ty,
        id: String,
        value: String,
    }

    impl serde::Serialize for Entry {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::ser::Serializer,
        {
            use serde::ser::SerializeStruct;

            let mut s = serializer.serialize_struct("Entry", 3)?;
            s.serialize_field(
                "ty",
                match self.ty {
                    Ty::A => "a",
                    Ty::B => "b",
                },
            )?;
            s.serialize_field("id", &self.id)?;
            s.serialize_field("value", &self.value)?;
            s.end()
        }
    }

    impl<'de> serde::Deserialize<'de> for Entry {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::de::Deserializer<'de>,
        {
            use serde::de::Error;

            let mut data = HashMap::<Cow<str>, String>::deserialize(deserializer)?;

            Ok(Entry {
                ty: match data
                    .remove("ty")
                    .ok_or_else(|| D::Error::custom("missing 'ty'"))?
                    .as_ref()
                {
                    "a" => Ty::A,
                    "b" => Ty::B,
                    other => return Err(D::Error::custom(format!("bad type '{other}'"))),
                },

                id: data
                    .remove("id")
                    .ok_or_else(|| D::Error::custom("missing 'id'"))?,
                value: data
                    .remove("value")
                    .ok_or_else(|| D::Error::custom("missing 'value'"))?,
            })
        }
    }

    const TYPE_SCHEMA: Schema = StringSchema::new("Type.")
        .format(&ApiStringFormat::Enum(&[
            EnumEntry {
                value: "a",
                description: "A",
            },
            EnumEntry {
                value: "b",
                description: "B",
            },
        ]))
        .schema();

    const PROPERTIES: ObjectSchema = ObjectSchema::new(
        "Stuff",
        &[
            ("id", false, &StringSchema::new("Some id.").schema()),
            ("ty", false, &TYPE_SCHEMA),
            ("value", false, &StringSchema::new("Some value.").schema()),
        ],
    );

    const ID_SCHEMA: Schema = StringSchema::new("ID schema.").min_length(3).schema();

    impl ApiSectionDataEntry for Entry {
        const INTERNALLY_TAGGED: Option<&'static str> = Some("ty");
        const SECION_CONFIG_USES_TYPE_KEY: bool = true;

        fn section_config() -> &'static SectionConfig {
            static SC: OnceLock<SectionConfig> = OnceLock::new();

            SC.get_or_init(|| {
                let mut config = SectionConfig::new(&ID_SCHEMA).with_type_key("ty");
                config.register_plugin(SectionConfigPlugin::new(
                    "a".to_string(),
                    Some("id".to_string()),
                    &PROPERTIES,
                ));
                config.register_plugin(SectionConfigPlugin::new(
                    "b".to_string(),
                    Some("id".to_string()),
                    &PROPERTIES,
                ));
                config
            })
        }

        fn section_type(&self) -> &'static str {
            match self.ty {
                Ty::A => "a",
                Ty::B => "a",
            }
        }
    }

    #[test]
    fn test_type_key() {
        let filename = "sync.cfg";
        let raw = "\
            a: first\n\
                \tvalue 1\n\
            \n\
            b: second\n\
                \tvalue 2\n\
        ";

        let parsed = Entry::section_config()
            .parse(filename, raw)
            .expect("failed to parse");
        let res: SectionConfigData<Entry> = parsed.try_into().expect("failed to convert");
        let written = Entry::write_section_config(filename, &res)
            .expect("failed to write out section config");
        assert_eq!(written, raw);
    }
}
