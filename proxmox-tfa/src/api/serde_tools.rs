//! Submodule for generic serde helpers.
//!
//! FIXME: This should appear in `proxmox-serde`.

use std::fmt;
use std::marker::PhantomData;

use serde::Deserialize;

/// Helper to abstract away serde details, see [`fold`](fold()).
pub struct FoldSeqVisitor<T, Out, F, Init>
where
    Init: FnOnce(Option<usize>) -> Out,
    F: Fn(&mut Out, T) -> (),
{
    init: Option<Init>,
    closure: F,
    expecting: &'static str,
    _ty: PhantomData<T>,
}

impl<T, Out, F, Init> FoldSeqVisitor<T, Out, F, Init>
where
    Init: FnOnce(Option<usize>) -> Out,
    F: Fn(&mut Out, T) -> (),
{
    pub fn new(expecting: &'static str, init: Init, closure: F) -> Self {
        Self {
            init: Some(init),
            closure,
            expecting,
            _ty: PhantomData,
        }
    }
}

impl<'de, T, Out, F, Init> serde::de::Visitor<'de> for FoldSeqVisitor<T, Out, F, Init>
where
    Init: FnOnce(Option<usize>) -> Out,
    F: Fn(&mut Out, T) -> (),
    T: Deserialize<'de>,
{
    type Value = Out;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(self.expecting)
    }

    fn visit_seq<A>(mut self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        // unwrap: this is the only place taking out init and we're consuming `self`
        let mut output = (self.init.take().unwrap())(seq.size_hint());

        while let Some(entry) = seq.next_element::<T>()? {
            (self.closure)(&mut output, entry);
        }

        Ok(output)
    }
}

/// Create a serde sequence visitor with simple callbacks.
///
/// This helps building things such as filters for arrays without having to worry about the serde
/// implementation details.
///
/// Example:
/// ```
/// # use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Test {
///     #[serde(deserialize_with = "stringify_u64")]
///     foo: Vec<String>,
/// }
///
/// fn stringify_u64<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
/// where
///     D: serde::Deserializer<'de>,
/// {
///     deserializer.deserialize_seq(proxmox_serde::fold(
///         "a sequence of integers",
///         |cap| cap.map(Vec::with_capacity).unwrap_or_else(Vec::new),
///         |out, num: u64| {
///             if num != 4 {
///                 out.push(num.to_string());
///             }
///         },
///     ))
/// }
///
/// let test: Test =
///     serde_json::from_str(r#"{"foo":[2, 4, 6]}"#).expect("failed to deserialize test");
/// assert_eq!(test.foo.len(), 2);
/// assert_eq!(test.foo[0], "2");
/// assert_eq!(test.foo[1], "6");
/// ```
pub fn fold<'de, T, Out, Init, Fold>(
    expected: &'static str,
    init: Init,
    fold: Fold,
) -> FoldSeqVisitor<T, Out, Fold, Init>
where
    Init: FnOnce(Option<usize>) -> Out,
    Fold: Fn(&mut Out, T) -> (),
    T: Deserialize<'de>,
{
    FoldSeqVisitor::new(expected, init, fold)
}
