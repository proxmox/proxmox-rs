use std::borrow::{Borrow, Cow};
use std::fmt;
use std::ops::Range;

/// Manage 2 lifetimes for deserializing.
///
/// When deserializing from a value it is considered to have lifetime `'de`. Any value that doesn't
/// need to live longer than the deserialized *input* can *borrow* from that lifetime.
///
/// For example, from the `String` `{ "hello": "you" }` you can deserialize a `HashMap<&'de str,
/// &'de str>`, as long as that map only exists as long as the original string.
///
/// However, if the data is `{ "hello": "\"hello\"" }`, then the value string needs to be
/// unescaped, and can only be owned. However, if you only need it *temporarily*, eg. to parse a
/// property string of numbers, you may want to avoid cloning individual parts from that.
///
/// Due to implementation details (particularly not wanting to provide a `Cow` version of
/// `PropertyIterator`), we may need to be able to hold references to such intermediate values.
///
/// For the above scenario, `'o` would be the original `'de` lifetime, and `'i` the intermediate
/// lifetime for the unescaped string.
///
/// Finally we also have an "Owned" value as a 3rd option.
pub enum Cow3<'o, 'i, B>
where
    B: 'o + 'i + ToOwned + ?Sized,
{
    /// Original lifetime from the deserialization entry point.
    Original(&'o B),

    /// Borrowed from an intermediate value.
    Intermediate(&'i B),

    /// Owned data.
    Owned(<B as ToOwned>::Owned),
}

impl<'o, 'i, B> Cow3<'o, 'i, B>
where
    B: 'o + 'i + ToOwned + ?Sized,
{
    /// From a `Cow` with the original lifetime.
    pub fn from_original<T>(value: T) -> Self
    where
        T: Into<Cow<'o, B>>,
    {
        match value.into() {
            Cow::Borrowed(v) => Self::Original(v),
            Cow::Owned(v) => Self::Owned(v),
        }
    }

    /// From a `Cow` with the intermediate lifetime.
    pub fn from_intermediate<T>(value: T) -> Self
    where
        T: Into<Cow<'i, B>>,
    {
        match value.into() {
            Cow::Borrowed(v) => Self::Intermediate(v),
            Cow::Owned(v) => Self::Owned(v),
        }
    }

    /// Turn into a `Cow`, forcing intermediate values to become owned.
    pub fn into_original_or_owned(self) -> Cow<'o, B> {
        match self {
            Self::Original(v) => Cow::Borrowed(v),
            Self::Intermediate(v) => Cow::Owned(v.to_owned()),
            Self::Owned(v) => Cow::Owned(v),
        }
    }
}

impl<'o, 'i, B> std::ops::Deref for Cow3<'o, 'i, B>
where
    B: 'o + 'i + ToOwned + ?Sized,
    <B as ToOwned>::Owned: Borrow<B>,
{
    type Target = B;

    fn deref(&self) -> &B {
        match self {
            Self::Original(v) => v,
            Self::Intermediate(v) => v,
            Self::Owned(v) => v.borrow(),
        }
    }
}

impl<'o, 'i, B> AsRef<B> for Cow3<'o, 'i, B>
where
    B: 'o + 'i + ToOwned + ?Sized,
    <B as ToOwned>::Owned: Borrow<B>,
{
    fn as_ref(&self) -> &B {
        self
    }
}

/// Build a `Cow3` with a value surviving the `'o` lifetime.
impl<'x, 'o, 'i, B> From<&'x B> for Cow3<'o, 'i, B>
where
    B: 'o + 'i + ToOwned + ?Sized,
    <B as ToOwned>::Owned: Borrow<B>,
    'x: 'o,
{
    fn from(value: &'x B) -> Self {
        Self::Original(value)
    }
}

impl<B: ?Sized> fmt::Display for Cow3<'_, '_, B>
where
    B: fmt::Display + ToOwned,
    B::Owned: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Original(ref b) => fmt::Display::fmt(b, f),
            Self::Intermediate(ref b) => fmt::Display::fmt(b, f),
            Self::Owned(ref o) => fmt::Display::fmt(o, f),
        }
    }
}

impl<B: ?Sized> fmt::Debug for Cow3<'_, '_, B>
where
    B: fmt::Debug + ToOwned,
    B::Owned: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Original(ref b) => fmt::Debug::fmt(b, f),
            Self::Intermediate(ref b) => fmt::Debug::fmt(b, f),
            Self::Owned(ref o) => fmt::Debug::fmt(o, f),
        }
    }
}

impl<'o, 'i> Cow3<'o, 'i, str> {
    /// Index value as a borrowed value.
    pub fn slice<'ni, I>(&'ni self, index: I) -> Cow3<'o, 'ni, str>
    where
        I: std::slice::SliceIndex<str, Output = str>,
        'i: 'ni,
    {
        match self {
            Self::Original(value) => Cow3::Original(&value[index]),
            Self::Intermediate(value) => Cow3::Intermediate(&value[index]),
            Self::Owned(value) => Cow3::Intermediate(&value.as_str()[index]),
        }
    }
}

pub fn str_slice_to_range(original: &str, slice: &str) -> Option<Range<usize>> {
    let bytes = original.as_bytes();

    let orig_addr = bytes.as_ptr() as usize;
    let slice_addr = slice.as_bytes().as_ptr() as usize;
    let offset = slice_addr.checked_sub(orig_addr)?;
    if offset > orig_addr + bytes.len() {
        return None;
    }

    let end = offset + slice.as_bytes().len();
    if end > orig_addr + bytes.len() {
        return None;
    }

    Some(offset..end)
}
