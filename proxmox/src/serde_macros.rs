#[macro_export]
macro_rules! forward_deserialize_to_from_str {
    ($typename:ty) => {
        impl<'de> serde::Deserialize<'de> for $typename {
            fn deserialize<D>(deserializer: D) -> Result<$typename, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                use serde::de::Error;

                struct ForwardToStrVisitor;

                impl<'a> serde::de::Visitor<'a> for ForwardToStrVisitor {
                    type Value = $typename;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str(concat!("a ", stringify!($typename)))
                    }

                    fn visit_str<E: Error>(self, v: &str) -> Result<$typename, E> {
                        v.parse::<$typename>()
                            .map_err(|err| Error::custom(err.to_string()))
                    }
                }

                deserializer.deserialize_str(ForwardToStrVisitor)
            }
        }
    }
}

#[macro_export]
macro_rules! forward_serialize_to_display {
    ($typename:ty) => {
        impl serde::Serialize for $typename {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::ser::Serializer,
            {
                serializer.serialize_str(&ToString::to_string(self))
            }
        }
    }
}
