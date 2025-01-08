//! This covers the type we use for flattened "PVE arrays", serializing as a map of prefixed
//! indices.

use std::collections::btree_map::{self, BTreeMap};
use std::fmt;
use std::marker::PhantomData;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use proxmox_schema::{Updater, UpdaterType};

/// Wraps a flattened PVE style "array" (list of indixed names in pve configs) in a sparse way by
/// using a [`BTreeMap`] for it internally and providing bounds-checked accessors.
#[derive(Clone, Debug)]
pub struct ArrayMap<T, const MAX: usize> {
    inner: BTreeMap<usize, T>,
}

impl<T, const MAX: usize> Default for ArrayMap<T, { MAX }> {
    fn default() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }
}

pub type Values<'a, T, const MAX: usize> = btree_map::Values<'a, usize, T>;
pub type ValuesMut<'a, T, const MAX: usize> = btree_map::ValuesMut<'a, usize, T>;

/// Indicates the index used to access a fixed length array is out of bounds.
#[derive(Default, Debug, Clone, Copy)]
pub struct OutOfBounds;

impl fmt::Display for OutOfBounds {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("index out of bounds")
    }
}

impl std::error::Error for OutOfBounds {}

impl<T, const MAX: usize> ArrayMap<T, { MAX }> {
    /// Create a new empty map.
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    /// Clear the map.
    pub fn clear(&mut self) {
        self.inner.clear()
    }

    /// Get an element via an index.
    pub fn get(&self, index: usize) -> Option<&T> {
        self.inner.get(&index)
    }

    /// Get mutable access to an element via an index.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.inner.get_mut(&index)
    }

    /// Insert an element at a specific index.
    ///
    /// Fails if the index is out of bounds.
    pub fn insert(&mut self, index: usize, item: T) -> Result<Option<T>, OutOfBounds> {
        if index >= MAX {
            Err(OutOfBounds)
        } else {
            Ok(self.inner.insert(index, item))
        }
    }

    /// Remove an element at a specific index.
    pub fn remove(&mut self, index: usize) -> Option<T> {
        self.inner.remove(&index)
    }

    /// Iterate through the values of the map.
    pub fn values(&self) -> Values<'_, T, { MAX }> {
        self.inner.values()
    }

    /// Iterate through mutable references of the values.
    pub fn values_mut(&mut self) -> ValuesMut<'_, T, { MAX }> {
        self.inner.values_mut()
    }

    fn lowest_unused_index(&self) -> Option<usize> {
        let mut cur = !0usize;
        for key in self.inner.keys().copied() {
            if key.wrapping_sub(1) != cur {
                return Some(key);
            }
            cur = key;
        }
        cur += 1;
        (cur < MAX).then_some(cur)
    }

    /// "Add" an element at the lowest unused index.
    pub fn add(&mut self, item: T) -> Option<usize> {
        let index = self.lowest_unused_index()?;
        self.inner.insert(index, item);
        Some(index)
    }

    /// Serialize this as a map where each index is prefixed with a specific static string.
    pub(crate) fn serialize<S>(
        &self,
        serializer: S,
        prefix: &'static str,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
        T: Serialize,
    {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(None)?;
        for (i, val) in self {
            map.serialize_entry(&format!("{prefix}{i}"), val)?;
        }
        map.end()
    }

    /// Deserialize as a struct with a specific prefix.
    ///
    /// The `prefix` must not change between calls to this function, as a list of element names is
    /// generated on first use to provide serde with all the names in advance.
    pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(
        de: D,
        prefix: &'static str,
        type_name: &'static str,
    ) -> Result<Self, D::Error>
    where
        T: Deserialize<'de>,
    {
        static FIELDS: OnceLock<Vec<String>> = OnceLock::new();
        static FIELDS_STR: OnceLock<Vec<&'static str>> = OnceLock::new();

        let fields_str: &'static [&'static str] = FIELDS_STR.get_or_init(|| {
            let fields = FIELDS.get_or_init(|| {
                let mut vec = Vec::with_capacity(MAX);
                for i in 0..MAX {
                    vec.push(format!("{prefix}{i}"));
                }
                vec
            });
            Vec::from_iter(fields.iter().map(|n| n.as_str()))
        });

        struct V<T, const MAX: usize> {
            prefix: &'static str,
            type_name: &'static str,
            _phantom: PhantomData<fn() -> T>,
        }

        impl<'de, T, const MAX: usize> serde::de::Visitor<'de> for V<T, { MAX }>
        where
            T: Deserialize<'de>,
        {
            type Value = ArrayMap<T, { MAX }>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a valid {}", self.type_name)
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                use serde::de::Error;

                let mut this = ArrayMap::default();

                while let Some((key, value)) = map.next_entry::<std::borrow::Cow<str>, T>()? {
                    if let Some(id) = key.as_ref().strip_prefix(self.prefix) {
                        if let Ok(id) = id.parse::<usize>() {
                            if this.insert(id, value).map_err(A::Error::custom)?.is_some() {
                                return Err(A::Error::custom(format!(
                                    "multiple '{}{id}' elements",
                                    self.prefix
                                )));
                            }
                            continue;
                        }
                    }
                    return Err(A::Error::custom(format!(
                        "invalid array element name {key}"
                    )));
                }

                Ok(this)
            }
        }

        de.deserialize_struct(
            type_name,
            fields_str,
            V::<T, { MAX }> {
                prefix,
                type_name,
                _phantom: PhantomData,
            },
        )
    }
}

impl<T, const MAX: usize> IntoIterator for ArrayMap<T, { MAX }> {
    type Item = (usize, T);
    type IntoIter = btree_map::IntoIter<usize, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

impl<'a, T, const MAX: usize> IntoIterator for &'a ArrayMap<T, { MAX }> {
    type Item = (&'a usize, &'a T);
    type IntoIter = btree_map::Iter<'a, usize, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}

impl<'a, T, const MAX: usize> IntoIterator for &'a mut ArrayMap<T, { MAX }> {
    type Item = (&'a usize, &'a mut T);
    type IntoIter = btree_map::IterMut<'a, usize, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter_mut()
    }
}

impl<T, const MAX: usize> Updater for ArrayMap<T, { MAX }> {
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl<T, const MAX: usize> UpdaterType for ArrayMap<T, { MAX }>
where
    T: UpdaterType,
{
    type Updater = ArrayMap<T::Updater, { MAX }>;
}
