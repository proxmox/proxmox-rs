//! Helper module for verifiers implemented via the api macro crate.
//!
//! We need this to seamlessly support verifying optional types. Consider this:
//!
//! ```ignore
//! type Annoying<T> = Option<T>;
//!
//! #[api({
//!     fields: {
//!         foo: {
//!             description: "Test",
//!             default: 2,
//!             minimum: 1,
//!             maximum: 5,
//!         },
//!         bar: {
//!             description: "Test",
//!             default: 2,
//!             minimum: 1,
//!             maximum: 5,
//!         },
//!     },
//! })]
//! struct Foo {
//!     foo: Option<usize>,
//!     bar: Annoying<usize>,
//! }
//! ```
//!
//! The macro does not know that `foo` and `bar` have in fact the same type, and wouldn't know that
//! in order to check `bar` it needs to first check the `Option`.
//!
//! With OIBITs or specialization, we could implement a trait that always gives us "the value we
//! actually want to check", but those aren't stable and guarded by a feature gate.
//!
//! So instead, we implement checks another way.

pub mod mark {
    pub struct Default;
    pub struct Special;
}

pub trait TestMinMax<Other> {
    fn test_minimum(&self, minimum: &Other) -> bool;
    fn test_maximum(&self, maximum: &Other) -> bool;
}

impl<Other> TestMinMax<Other> for Other
where
    Other: Ord,
{
    #[inline]
    fn test_minimum(&self, minimum: &Other) -> bool {
        *self >= *minimum
    }

    #[inline]
    fn test_maximum(&self, maximum: &Other) -> bool {
        *self <= *maximum
    }
}

impl<Other> TestMinMax<Other> for Option<Other>
where
    Other: Ord,
{
    #[inline]
    fn test_minimum(&self, minimum: &Other) -> bool {
        self.as_ref().map(|x| *x >= *minimum).unwrap_or(true)
    }

    #[inline]
    fn test_maximum(&self, maximum: &Other) -> bool {
        self.as_ref().map(|x| *x <= *maximum).unwrap_or(true)
    }
}
