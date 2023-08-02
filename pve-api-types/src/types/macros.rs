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
        #[derive(Debug, serde::Deserialize, serde::Serialize)]
        pub struct $type_name {
            $(
                #[serde(skip_serializing_if = "Option::is_none")]
                $field_names: Option<$field_type>,
            )+
        }
    };
}

pub(crate) use generate_array_field;
