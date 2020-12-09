/// Given a type which implements [`FromStr`](std::str::FromStr), derive
/// [`Deserialize`](serde::Deserialize) by using the [`from_str`](std::str::FromStr::from_str())
/// method.
///
/// ```
/// # use std::str::FromStr;
/// # use anyhow::bail;
/// # use proxmox::forward_deserialize_to_from_str;
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
    };
}

/// Given a type which implements [`Display`], derive [`Serialize`](serde::Serialize) by using the
/// [`Display`] trait.
///
/// ```
/// # use std::fmt;
/// # use proxmox::forward_serialize_to_display;
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
        impl serde::Serialize for $typename {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::ser::Serializer,
            {
                serializer.serialize_str(&ToString::to_string(self))
            }
        }
    };
}
