//! Some parsing helpers for the PVE API, mainly to deal with perl's untypedness.

use std::fmt;

use serde::de::Unexpected;

// Boolean:

pub trait FromBool: Sized + Default {
    fn from_bool(value: bool) -> Self;
}

impl FromBool for bool {
    fn from_bool(value: bool) -> Self {
        value
    }
}

impl FromBool for Option<bool> {
    fn from_bool(value: bool) -> Self {
        Some(value)
    }
}

pub fn deserialize_bool<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: FromBool,
{
    deserializer.deserialize_any(BoolVisitor::<T>::new())
}

struct BoolVisitor<T>(std::marker::PhantomData<T>);

impl<T> BoolVisitor<T> {
    fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<'de, T: FromBool> serde::de::DeserializeSeed<'de> for BoolVisitor<T> {
    type Value = T;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize_bool(deserializer)
    }
}

impl<'de, T> serde::de::Visitor<'de> for BoolVisitor<T>
where
    T: FromBool,
{
    type Value = T;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a boolean-ish...")
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Default::default())
    }

    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Self::Value::from_bool(value))
    }

    fn visit_i128<E: serde::de::Error>(self, value: i128) -> Result<Self::Value, E> {
        Ok(Self::Value::from_bool(value != 0))
    }

    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Self::Value::from_bool(value != 0))
    }

    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Self::Value::from_bool(value != 0))
    }

    fn visit_u128<E: serde::de::Error>(self, value: u128) -> Result<Self::Value, E> {
        Ok(Self::Value::from_bool(value != 0))
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
        let value = if value.eq_ignore_ascii_case("true")
            || value.eq_ignore_ascii_case("yes")
            || value.eq_ignore_ascii_case("on")
        {
            true
        } else if value.eq_ignore_ascii_case("false")
            || value.eq_ignore_ascii_case("no")
            || value.eq_ignore_ascii_case("off")
        {
            false
        } else {
            return Err(E::invalid_value(
                serde::de::Unexpected::Str(value),
                &"a boolean-like value",
            ));
        };
        Ok(Self::Value::from_bool(value))
    }
}

// integer helpers:

macro_rules! integer_helper {
    ($ty:ident, $deserialize_name:ident, $trait: ident, $from_name:ident, $visitor:ident) => {
        #[doc(hidden)]
        pub trait $trait: Sized + Default {
            fn $from_name(value: $ty) -> Self;
        }

        impl $trait for $ty {
            fn $from_name(value: $ty) -> Self {
                value
            }
        }

        impl $trait for Option<$ty> {
            fn $from_name(value: $ty) -> Self {
                Some(value)
            }
        }

        pub fn $deserialize_name<'de, D, T>(deserializer: D) -> Result<T, D::Error>
        where
            D: serde::Deserializer<'de>,
            T: $trait,
        {
            deserializer.deserialize_any($visitor::<T>::new())
        }

        struct $visitor<T>(std::marker::PhantomData<T>);

        impl<T> $visitor<T> {
            fn new() -> Self {
                Self(std::marker::PhantomData)
            }
        }

        impl<'de, T: $trait> serde::de::DeserializeSeed<'de> for $visitor<T> {
            type Value = T;

            fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                $deserialize_name(deserializer)
            }
        }

        impl<'de, T> serde::de::Visitor<'de> for $visitor<T>
        where
            T: $trait,
        {
            type Value = T;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(concat!("a ", stringify!($ty), "-ish..."))
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                deserializer.deserialize_any(self)
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(Default::default())
            }

            fn visit_i128<E: serde::de::Error>(self, value: i128) -> Result<Self::Value, E> {
                $ty::try_from(value)
                    .map_err(|_| E::invalid_value(Unexpected::Other("i128"), &self))
                    .map(Self::Value::$from_name)
            }

            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
                $ty::try_from(value)
                    .map_err(|_| E::invalid_value(Unexpected::Signed(value), &self))
                    .map(Self::Value::$from_name)
            }

            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
                $ty::try_from(value)
                    .map_err(|_| E::invalid_value(Unexpected::Unsigned(value), &self))
                    .map(Self::Value::$from_name)
            }

            fn visit_u128<E: serde::de::Error>(self, value: u128) -> Result<Self::Value, E> {
                $ty::try_from(value)
                    .map_err(|_| E::invalid_value(Unexpected::Other("u128"), &self))
                    .map(Self::Value::$from_name)
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                let value = value
                    .parse()
                    .map_err(|_| E::invalid_value(Unexpected::Str(value), &self))?;
                self.visit_i64(value)
            }
        }
    };
}

integer_helper!(
    isize,
    deserialize_isize,
    FromIsize,
    from_isize,
    IsizeVisitor
);

integer_helper!(
    usize,
    deserialize_usize,
    FromUsize,
    from_usize,
    UsizeVisitor
);

integer_helper!(u8, deserialize_u8, FromU8, from_u8, U8Visitor);
integer_helper!(u16, deserialize_u16, FromU16, from_u16, U16Visitor);
integer_helper!(u32, deserialize_u32, FromU32, from_u32, U32Visitor);
integer_helper!(u64, deserialize_u64, FromU64, from_u64, U64Visitor);
integer_helper!(i8, deserialize_i8, FromI8, from_i8, I8Visitor);
integer_helper!(i16, deserialize_i16, FromI16, from_i16, I16Visitor);
integer_helper!(i32, deserialize_i32, FromI32, from_i32, I32Visitor);
integer_helper!(i64, deserialize_i64, FromI64, from_i64, I64Visitor);

// float helpers:

macro_rules! float_helper {
    ($ty:ident, $deserialize_name:ident, $visitor:ident) => {
        pub fn $deserialize_name<'de, D, T>(deserializer: D) -> Result<T, D::Error>
        where
            D: serde::Deserializer<'de>,
            T: FromF64,
        {
            deserializer.deserialize_any($visitor::<T>::new())
        }

        struct $visitor<T>(std::marker::PhantomData<T>);

        impl<T> $visitor<T> {
            fn new() -> Self {
                Self(std::marker::PhantomData)
            }
        }

        impl<'de, T: FromF64> serde::de::DeserializeSeed<'de> for $visitor<T> {
            type Value = T;

            fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                $deserialize_name(deserializer)
            }
        }

        impl<'de, T> serde::de::Visitor<'de> for $visitor<T>
        where
            T: FromF64,
        {
            type Value = T;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(concat!("a ", stringify!($ty), "-ish..."))
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                deserializer.deserialize_any(self)
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(Default::default())
            }

            fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Self::Value, E> {
                Ok(T::from_f64(value))
            }

            fn visit_i128<E: serde::de::Error>(self, value: i128) -> Result<Self::Value, E> {
                let conv = value as f64;
                if conv as i128 == value {
                    Ok(T::from_f64(conv))
                } else {
                    Err(E::invalid_value(Unexpected::Other("i128"), &self))
                }
            }

            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
                let conv = value as f64;
                if conv as i64 == value {
                    Ok(T::from_f64(conv))
                } else {
                    Err(E::invalid_value(Unexpected::Signed(value), &self))
                }
            }

            fn visit_u128<E: serde::de::Error>(self, value: u128) -> Result<Self::Value, E> {
                let conv = value as f64;
                if conv as u128 == value {
                    Ok(T::from_f64(conv))
                } else {
                    Err(E::invalid_value(Unexpected::Other("u128"), &self))
                }
            }

            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
                let conv = value as f64;
                if conv as u64 == value {
                    Ok(T::from_f64(conv))
                } else {
                    Err(E::invalid_value(Unexpected::Unsigned(value), &self))
                }
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                let value = value
                    .parse()
                    .map_err(|_| E::invalid_value(Unexpected::Str(value), &self))?;
                self.visit_f64(value)
            }
        }
    };
}

#[doc(hidden)]
pub trait FromF64: Sized + Default {
    fn from_f64(value: f64) -> Self;
}

impl FromF64 for f32 {
    #[inline(always)]
    fn from_f64(f: f64) -> f32 {
        f as f32
    }
}

impl FromF64 for f64 {
    #[inline(always)]
    fn from_f64(f: f64) -> f64 {
        f
    }
}

impl<T: FromF64> FromF64 for Option<T> {
    #[inline(always)]
    fn from_f64(f: f64) -> Option<T> {
        Some(T::from_f64(f))
    }
}

float_helper!(f32, deserialize_f32, F32Visitor);
float_helper!(f64, deserialize_f64, F64Visitor);
