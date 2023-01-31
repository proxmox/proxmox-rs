//! The "basic" api types we generally require along with some of their macros.

use crate::{ApiStringFormat, Schema, StringSchema};

#[rustfmt::skip]
#[macro_export]
macro_rules! SAFE_ID_REGEX_STR { () => { r"(?:[A-Za-z0-9_][A-Za-z0-9._\-]*)" }; }

const_regex! {
    /// Regex for safe identifiers.
    ///
    /// This
    /// [article](https://dwheeler.com/essays/fixing-unix-linux-filenames.html)
    /// contains further information why it is reasonable to restict
    /// names this way. This is not only useful for filenames, but for
    /// any identifier command line tools work with.
    pub SAFE_ID_REGEX = concat!(r"^", SAFE_ID_REGEX_STR!(), r"$");
    pub PASSWORD_REGEX = r"^[[:^cntrl:]]*$"; // everything but control characters
}

pub const SAFE_ID_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&SAFE_ID_REGEX);
pub const PASSWORD_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&PASSWORD_REGEX);

pub const PASSWORD_SCHEMA: Schema = StringSchema::new("Password.")
    .format(&PASSWORD_FORMAT)
    .min_length(1)
    .max_length(1024)
    .schema();
