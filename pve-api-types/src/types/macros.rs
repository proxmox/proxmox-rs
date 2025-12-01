use proxmox_schema::SchemaPropertyEntry;

pub(crate) const __DIGIT_SPACE: usize = 4;

/// Since our object schemas need lexicographically sorted names, we generate *those* indices
/// separately.
///
/// The idea is as follows:
/// - If we can attach a zero without going out of bounds, that's the next number.
///   (1 => 10 => 100 => 1000, and so on)
/// - Otherwise, repeat until we end up at zero (which is the end):
///   - If the number does not end in a `9`, we can just increment. If we don't exceed the limit,
///     return the number.
///     (3 => 4, 134 => 135, 3850 => 3851, and so on)
///
///   - If it does end with a `9`, cut it off:
///     (1299 => 129 => 12, 14399 => 1439 => 143)
const fn next_lexicographical_number(mut at: usize, count: usize) -> Option<usize> {
    // special case since `0 * 10` is still 0 ;-)
    if at == 0 {
        return Some(1);
    }

    let longer = at * 10;
    if longer < count {
        return Some(longer);
    }

    while at != 0 {
        if at % 10 != 9 {
            at += 1;
            if at < count {
                return Some(at);
            }
        }
        at /= 10;
    }

    None
}

/// Equivalent to `write!(to, "{name}{index}")`.
pub(crate) const fn write_name_index(to: &mut [u8], name: &'static str, mut index: usize) {
    let name = name.as_bytes();
    let mut len = 0;
    while len != name.len() {
        to[len] = name[len];
        len += 1;
    }
    if index == 0 {
        to[len] = b'0';
        len += 1;
    } else {
        let mut digits = 0;
        let mut copy = index;
        while copy != 0 {
            digits += 1;
            copy /= 10;
        }
        len += digits;

        let mut at = len - 1;
        while index != 0 {
            to[at] = b'0' + (index % 10) as u8;
            index /= 10;
            at -= 1;
        }
    }
}

/// Fill the buffer in `data` with `prefix0`, `prefix1`, `prefix2`, ... - but sorted
/// lexicographically!
pub(crate) const fn __fill_names<const N: usize>(prefix: &'static str, data: &mut [u8]) {
    let unit_size = __DIGIT_SPACE + prefix.len();

    let mut item = 0;
    let mut sorted_index = Some(0);
    while item != N {
        let at = item * unit_size;

        let (_, slot) = data.split_at_mut(at);
        match sorted_index {
            None => panic!("ran out of indices"),
            Some(index) => {
                write_name_index(slot, prefix, index);
                sorted_index = next_lexicographical_number(index, N);
            }
        }

        item += 1;
    }
}

/// Assuming `data` is now an array of field names, perform the equivalent of:
///
/// `properties[N].0 = fields[N] foreach N;`
pub(crate) const fn __fill_properties<const N: usize>(
    prefix: &'static str,
    mut data: &'static [u8],
    properties: &mut [SchemaPropertyEntry; N],
) {
    let unit_size = __DIGIT_SPACE + prefix.len();
    let mut item = 0;
    while item != N {
        let slot;
        (slot, data) = data.split_at(unit_size);
        let mut len = 0;
        while len != unit_size && slot[len] != 0 {
            len += 1;
        }
        let slot = slot.split_at(len).0;

        match std::str::from_utf8(slot) {
            Ok(field_name) => properties[item].0 = field_name,
            Err(_) => panic!("non utf-8 field"),
        }

        item += 1;
    }
}

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

            const ITEM_SCHEMA: ::proxmox_schema::Schema =
                ::proxmox_api_macro::json_schema! $api_def ;

            const ARRAY_OBJECT_SCHEMA: Schema = const {
                const BUFSIZE: usize = (stringify!($field_prefix).len() + $crate::types::__DIGIT_SPACE) * $array_len;

                const NAMES: [u8; BUFSIZE] = const {
                    let mut buffer = [0u8; BUFSIZE];
                    $crate::types::__fill_names::<$array_len>(stringify!($field_prefix), &mut buffer);
                    buffer
                };

                const PROPERTIES: [::proxmox_schema::SchemaPropertyEntry; $array_len] = const {
                    let mut properties = [("", false, &$type_name::ITEM_SCHEMA); $array_len];
                    $crate::types::__fill_properties::<$array_len>(stringify!($field_prefix), &NAMES, &mut properties);
                    properties
                };

                ::proxmox_schema::ObjectSchema::new(
                    concat!("Container for the `", stringify!($field_prefix), "[N]` fields."),
                    &PROPERTIES,
                ).schema()
            };
        }

        impl ::proxmox_schema::ApiType for $type_name {
            const API_SCHEMA: ::proxmox_schema::Schema = Self::ARRAY_OBJECT_SCHEMA;
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
