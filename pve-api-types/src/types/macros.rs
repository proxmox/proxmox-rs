macro_rules! generate_array_field {
    ($type_name:ident [ $array_len:expr ] :
     $doc:expr,
     $field_type:ty => $api_def:tt
     $field_prefix:ident
    ) => {
        #[doc = concat!("Container for the `", stringify!($field_prefix), "[N]` fields.")]
        #[derive(Debug, Default)]
        pub struct $type_name {
            inner: crate::array::ArrayMap<$field_type, { $array_len }>,
        }

        impl $type_name {
            pub const MAX: usize = $array_len;
        }

        impl ::proxmox_schema::ApiType for $type_name {
            const API_SCHEMA: ::proxmox_schema::Schema =
                ::proxmox_api_macro::json_schema! $api_def ;
        }

        impl std::ops::Deref for $type_name {
            type Target = crate::array::ArrayMap<$field_type, { $array_len }>;

            fn deref(&self) -> &Self::Target {
                &self.inner
            }
        }

        impl std::ops::DerefMut for $type_name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.inner
            }
        }

        impl serde::Serialize for $type_name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                self.inner.serialize(serializer, stringify!($field_prefix))
            }
        }

        impl<'de> serde::Deserialize<'de> for $type_name {
            fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
                use std::sync::OnceLock;

                static FIELDS: OnceLock<Vec<String>> = OnceLock::new();
                static FIELDS_STR: OnceLock<Vec<&'static str>> = OnceLock::new();

                let fields_str: &'static [&'static str] = FIELDS_STR.get_or_init(|| {
                    let fields = FIELDS.get_or_init(|| {
                        let mut vec = Vec::with_capacity($array_len);
                        for i in 0..$array_len {
                            vec.push(format!("{}{}", stringify!($field_prefix), i));
                        }
                        vec
                    });
                    Vec::from_iter(fields.iter().map(|n| n.as_str()))
                });


                let inner = crate::array::ArrayMap::deserialize(
                    de,
                    stringify!($field_prefix),
                    stringify!($type_name),
                    fields_str,
                )?;
                Ok(Self { inner })
            }
        }

        impl IntoIterator for $type_name {
            type Item = <crate::array::ArrayMap::<$field_type, { $array_len }> as IntoIterator>::Item;
            type IntoIter = <crate::array::ArrayMap::<$field_type, { $array_len }> as IntoIterator>::IntoIter;
            fn into_iter(self) -> Self::IntoIter {
                self.inner.into_iter()
            }
        }

        impl<'a> IntoIterator for &'a $type_name {
            type Item = <&'a crate::array::ArrayMap::<$field_type, { $array_len }> as IntoIterator>::Item;
            type IntoIter = <&'a crate::array::ArrayMap::<$field_type, { $array_len }> as IntoIterator>::IntoIter;
            fn into_iter(self) -> Self::IntoIter {
                (&self.inner).into_iter()
            }
        }

        impl<'a> IntoIterator for &'a mut $type_name {
            type Item = <&'a mut crate::array::ArrayMap::<$field_type, { $array_len }> as IntoIterator>::Item;
            type IntoIter = <&'a mut crate::array::ArrayMap::<$field_type, { $array_len }> as IntoIterator>::IntoIter;
            fn into_iter(self) -> Self::IntoIter {
                (&mut self.inner).into_iter()
            }
        }
    };
}

pub(crate) use generate_array_field;
