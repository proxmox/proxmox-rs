use serde::Serializer;
use serde_json::Value;

/// This defines a *fixed* serializer (iow. also where/how to write out the data).
///
/// (Note that `serde::Serializer` is implemented for `__&mut__ serde_json::Serializer`.
type SenderSerializer<'a> = &'a mut serde_json::Serializer<
    &'a mut std::io::BufWriter<proxmox_async::blocking::SenderWriter>,
>;

/// This is an object-safe trait which requires the ability to serialize into particular
/// Serializer instances.
pub trait SerializableReturn {
    /// Serializes self into a [`proxmox_async::blocking::SenderWriter`] wrapped
    /// into a [`std::io::BufWriter`]
    ///
    /// If `value` is an Object/Map, serializes that first and puts the value of
    /// `self` into the `data` property.
    fn sender_serialize(
        &self,
        serializer: SenderSerializer,
        value: Value,
    ) -> Result<
        <SenderSerializer<'_> as serde::Serializer>::Ok,
        <SenderSerializer<'_> as serde::Serializer>::Error,
    >;

    /// Returns a value again from self
    fn to_value(&self) -> Result<Value, serde_json::error::Error>;
}

impl<T> SerializableReturn for T
where
    T: serde::Serialize,
{
    fn sender_serialize(
        &self,
        serializer: SenderSerializer,
        value: Value,
    ) -> Result<
        <SenderSerializer<'_> as serde::Serializer>::Ok,
        <SenderSerializer<'_> as serde::Serializer>::Error,
    > {
        use serde::ser::SerializeMap;
        if let Some(original) = value.as_object() {
            let mut map = serializer.serialize_map(None)?;
            for (k, v) in original {
                map.serialize_entry(k, v)?;
            }

            map.serialize_key("data")?;
            map.serialize_value(&self)?;
            map.end()
        } else {
            self.serialize(serializer)
        }
    }

    fn to_value(&self) -> Result<Value, serde_json::error::Error> {
        serde_json::to_value(self)
    }
}
