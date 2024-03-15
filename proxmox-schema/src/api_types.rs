//! The "basic" api types we generally require along with some of their macros.
use const_format::concatcp;

use crate::{ApiStringFormat, Schema, StringSchema};

#[rustfmt::skip]
const SAFE_ID_REGEX_STR: &str = r"(?:[A-Za-z0-9_][A-Za-z0-9._\-]*)";

const_regex! {
    /// Regex for safe identifiers.
    ///
    /// This
    /// [article](https://dwheeler.com/essays/fixing-unix-linux-filenames.html)
    /// contains further information why it is reasonable to restict
    /// names this way. This is not only useful for filenames, but for
    /// any identifier command line tools work with.
    pub SAFE_ID_REGEX = concatcp!(r"^", SAFE_ID_REGEX_STR, r"$");
    /// Password. Allow everything but control characters.
    pub PASSWORD_REGEX = r"^[[:^cntrl:]]*$";
    /// Single line comment. Allow everything but control characters.
    pub SINGLE_LINE_COMMENT_REGEX = r"^[[:^cntrl:]]*$";
}

pub const SAFE_ID_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&SAFE_ID_REGEX);
pub const PASSWORD_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&PASSWORD_REGEX);
pub const SINGLE_LINE_COMMENT_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&SINGLE_LINE_COMMENT_REGEX);

pub const PASSWORD_SCHEMA: Schema = StringSchema::new("Password.")
    .format(&PASSWORD_FORMAT)
    .min_length(1)
    .max_length(1024)
    .schema();

pub const COMMENT_SCHEMA: Schema = StringSchema::new("Comment.")
    .format(&SINGLE_LINE_COMMENT_FORMAT)
    .max_length(128)
    .schema();
