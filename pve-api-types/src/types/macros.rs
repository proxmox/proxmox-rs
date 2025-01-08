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
                let inner = crate::array::ArrayMap::deserialize(
                    de,
                    stringify!($field_prefix),
                    stringify!($type_name),
                )?;
                Ok(Self { inner })
            }
        }
    };
}

pub(crate) use generate_array_field;
