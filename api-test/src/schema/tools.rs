//! Helper module to perform the same format checks on various string types.
//!
//! This is used for formats which should be checked on strings, arrays of strings, and optional
//! variants of both.

use std::collections::HashSet;

/// Allows testing predicates on all the contained strings of a type.
pub trait StringContainer {
    fn all<F: Fn(&str) -> bool>(&self, pred: F) -> bool;
}

impl StringContainer for String {
    fn all<F: Fn(&str) -> bool>(&self, pred: F) -> bool {
        pred(&self)
    }
}

impl StringContainer for Option<String> {
    fn all<F: Fn(&str) -> bool>(&self, pred: F) -> bool {
        match self {
            Some(ref v) => pred(v),
            None => true,
        }
    }
}

impl StringContainer for Vec<String> {
    fn all<F: Fn(&str) -> bool>(&self, pred: F) -> bool {
        self.iter().all(|s| pred(&s))
    }
}

impl StringContainer for Option<Vec<String>> {
    fn all<F: Fn(&str) -> bool>(&self, pred: F) -> bool {
        self.as_ref().map(|c| StringContainer::all(c, pred)).unwrap_or(true)
    }
}

impl StringContainer for HashSet<String> {
    fn all<F: Fn(&str) -> bool>(&self, pred: F) -> bool {
        self.iter().all(|s| pred(s))
    }
}

impl StringContainer for Option<HashSet<String>> {
    fn all<F: Fn(&str) -> bool>(&self, pred: F) -> bool {
        self.as_ref().map(|c| StringContainer::all(c, pred)).unwrap_or(true)
    }
}
