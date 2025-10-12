/// Given a type which implements [`FromStr`](std::str::FromStr), derive
/// [`Deserialize`](serde::Deserialize) by using the [`from_str`](std::str::FromStr::from_str())
/// method.
///
/// ```
/// # use std::str::FromStr;
/// # use anyhow::bail;
/// # use proxmox_serde::forward_deserialize_to_from_str;
/// struct AsciiAlnum(String);
///
/// impl FromStr for AsciiAlnum {
///     type Err = anyhow::Error;
///     fn from_str(s: &str) -> Result<Self, Self::Err> {
///         if s.as_bytes().iter().any(|&b| !b.is_ascii_alphanumeric()) {
///             bail!("invalid non-ascii-alphanumeric characters in string");
///         }
///         Ok(Self(s.to_string()))
///     }
/// }
///
/// forward_deserialize_to_from_str!(AsciiAlnum);
/// ```
#[macro_export]
macro_rules! forward_deserialize_to_from_str {
    ($typename:ty) => {
        impl<'de> ::serde::Deserialize<'de> for $typename {
            fn deserialize<D>(deserializer: D) -> Result<$typename, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                ::std::borrow::Cow::<'de, str>::deserialize(deserializer)?
                    .parse()
                    .map_err(::serde::de::Error::custom)
            }
        }
    };
}

/// Given a type which implements [`Display`], derive [`Serialize`](serde::Serialize) by using the
/// [`Display`] trait.
///
/// ```
/// # use std::fmt;
/// # use proxmox_serde::forward_serialize_to_display;
/// struct DoubleAngleBracketed(String);
///
/// impl fmt::Display for DoubleAngleBracketed {
///     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
///         write!(f, "<<{}>>", self.0)
///     }
/// }
///
/// forward_serialize_to_display!(DoubleAngleBracketed);
/// ```
///
/// [`Display`]: std::fmt::Display
#[macro_export]
macro_rules! forward_serialize_to_display {
    ($typename:ty) => {
        impl ::serde::Serialize for $typename {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ::serde::ser::Serializer,
            {
                serializer.collect_str(self)
            }
        }
    };
}

/// Given a type which derives [`Deserialize`], derive [`FromStr`][FromStr] by using
/// serde's built-in [`StrDeserializer`][StrDe] via the
/// [`IntoDeserializer`][IntoDe] trait.
///
/// [FromStr]: std::str::FromStr
/// [StrDe]: serde::de::value::StrDeserializer
/// [IntoDe]: serde::de::value::IntoDeserializer
///
/// ```
/// # use serde::Deserialize;
/// # use proxmox_serde::forward_from_str_to_deserialize;
///
/// #[derive(Clone, Debug, Deserialize)]
/// enum AnEnum {
///     Apples,
///     Bananas,
///     Tomatoes,
/// }
///
/// forward_from_str_to_deserialize!(AnEnum);
/// ```
#[macro_export]
macro_rules! forward_from_str_to_deserialize {
    ($typename:ty) => {
        impl std::str::FromStr for $typename {
            type Err = serde::de::value::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                <Self as ::serde::Deserialize>::deserialize(
                    serde::de::IntoDeserializer::into_deserializer(s),
                )
            }
        }
    };
}

/// Given a type which derives [`Serialize`], derive [`Display`](std::fmt::Display) by using the
/// [`Formatter`](std::fmt::Formatter) directly as a [`Serializer`](serde::Serializer).
///
/// ```
/// # use serde::Serialize;
/// # use proxmox_serde::forward_display_to_serialize;
///
/// #[derive(Clone, Debug, Serialize)]
/// enum AnEnum {
///     Apples,
///     Bananas,
///     Tomatoes,
/// }
///
/// forward_display_to_serialize!(AnEnum);
/// ```
#[macro_export]
macro_rules! forward_display_to_serialize {
    ($typename:ty) => {
        impl ::std::fmt::Display for $typename {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                ::serde::Serialize::serialize(self, f)
            }
        }
    };
}
