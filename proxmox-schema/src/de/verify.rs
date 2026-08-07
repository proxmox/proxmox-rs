use serde::de::{self, Deserialize};

use super::Schema;

/// Helper to collect multiple deserialization errors for better reporting.
///
/// This is similar to [`IgnoredAny`](serde::de::IgnoredAny) in that it implements the helper copy
/// of serde's `DeserializeSeed` trait
/// ([`DeserializeSeedOrVerify`](super::DeserializeSeedOrVerify)), but does not actually
/// deserialize to anything, however, when a deserialization error occurs, it'll try to continue
/// and collect further errors.
///
/// This only makes sense with the [`SchemaDeserializer`](super::SchemaDeserializer).
pub(super) struct Verifier;

#[cfg_attr(
    proxmox_schema = "strict",
    deprecated(note = "moved to proxmox_schema::de::verify")
)]
pub fn verify(schema: &'static Schema, value: &str) -> Result<(), anyhow::Error> {
    super::verify(schema, value)
}

pub(super) struct Visitor;

impl<'de> super::DeserializeSeedOrVerify<'de> for Visitor {
    type Value = Verifier;

    fn deserialize(
        self,
        deserializer: super::SchemaDeserializer<'de, '_, '_>,
    ) -> Result<Self::Value, super::Error> {
        deserializer.de_any(self)
    }

    fn deserialize_no_schema(
        self,
        deserializer: super::no_schema::NoSchemaDeserializer<'de, '_>,
    ) -> Result<Self::Value, super::Error> {
        de::IgnoredAny::deserialize(deserializer)?;
        Ok(Verifier)
    }
}

impl<'de> super::VisitorOrVerify<'de> for Visitor {
    type Value = Verifier;

    fn visit_bool(self, _: bool) -> Result<Self::Value, super::Error> {
        Ok(Verifier)
    }

    fn visit_i64(self, _: i64) -> Result<Self::Value, super::Error> {
        Ok(Verifier)
    }

    fn visit_u64(self, _: u64) -> Result<Self::Value, super::Error> {
        Ok(Verifier)
    }

    fn visit_f64(self, _: f64) -> Result<Self::Value, super::Error> {
        Ok(Verifier)
    }

    fn visit_str(self, _: &str) -> Result<Self::Value, super::Error> {
        Ok(Verifier)
    }

    fn visit_borrowed_str(self, _: &'de str) -> Result<Self::Value, super::Error> {
        Ok(Verifier)
    }

    fn visit_string(self, _: String) -> Result<Self::Value, super::Error> {
        Ok(Verifier)
    }

    fn visit_seq(
        self,
        mut seq: super::SeqAccess<'de, '_, '_, '_>,
    ) -> Result<Self::Value, super::Error> {
        loop {
            match seq.next(Visitor) {
                Ok(None) => break,
                Ok(Some(_)) => (),
                Err(err) if err.is_recoverable() => (),
                Err(err) => return Err(err),
            };
        }

        Ok(Verifier)
    }

    fn visit_map(self, mut map: super::MapAccess<'de, '_>) -> Result<Self::Value, super::Error> {
        loop {
            match map.get_next_key_seed(std::marker::PhantomData::<String>) {
                Ok(None) => break,
                Ok(Some(_)) => match map.get_next_value_seed(Visitor) {
                    Ok(_) => (),
                    Err(err) if err.is_recoverable() => (),
                    Err(err) => return Err(err),
                },
                Err(err) if err.is_recoverable() => (),
                Err(err) => return Err(err),
            }
        }

        Ok(Verifier)
    }
}
