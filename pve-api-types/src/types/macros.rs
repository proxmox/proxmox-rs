macro_rules! generate_array_field {
    ($type_name:ident :
     $(#[$doc:meta])*
     $field_type:ty => $api_def:tt
     $($field_names:ident),+ $(,)?) => {
        #[api(
            properties: {
                $( $field_names: $api_def, )*
            },
        )]
        $(#[$doc])*
        #[derive(Debug, Default, serde::Serialize)]
        pub struct $type_name {
            $(
                #[serde(skip_serializing_if = "Option::is_none")]
                $field_names: Option<$field_type>,
            )+
        }
        impl<'de> serde::Deserialize<'de> for $type_name {
            fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
                const FIELDS: &'static [&'static str] = &[$(stringify!($field_names),)+];
                struct V;
                impl<'de> serde::de::Visitor<'de> for V {
                    type Value = $type_name;

                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        f.write_str(concat!("a valid ", stringify!($type_name)))
                    }

                    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                    where
                        A: serde::de::MapAccess<'de>,
                    {
                        let mut this = $type_name::default();
                        while let Some((key, value)) = map.next_entry::<std::borrow::Cow<str>, String>()? {
                            match key.as_ref() {
                                $(
                                    stringify!($field_names) => this.$field_names = Some(value),
                                )+
                                _ => {
                                    use serde::de::Error;
                                    return Err(A::Error::custom(
                                        "invalid element in array helper struct"
                                    ));
                                }
                            }
                        }

                        Ok(this)
                    }
                }
                de.deserialize_struct(stringify!($type_name), FIELDS, V)
            }
        }
    };
}

pub(crate) use generate_array_field;
