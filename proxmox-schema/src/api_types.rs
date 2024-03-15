//! The "basic" api types we generally require along with some of their macros.
use const_format::concatcp;

use crate::{ApiStringFormat, Schema, StringSchema};

#[rustfmt::skip]
const IPV4OCTET: &str = r"(?:25[0-5]|(?:2[0-4]|1[0-9]|[1-9])?[0-9])";

#[rustfmt::skip]
const IPV6H16: &str = r"(?:[0-9a-fA-F]{1,4})";

/// Returns the regular expression string to match IPv4 addresses
#[rustfmt::skip]
pub const IPV4RE_STR: &str = concatcp!(r"(?:(?:", IPV4OCTET, r"\.){3}", IPV4OCTET, ")");

#[rustfmt::skip]
const IPV6LS32: &str = concatcp!(r"(?:(?:", IPV4RE_STR, "|", IPV6H16, ":", IPV6H16, "))" );

/// Returns the regular expression string to match IPv6 addresses
#[rustfmt::skip]
pub const IPV6RE_STR: &str = concatcp!(r"(?:",
    r"(?:(?:",                                         r"(?:", IPV6H16, r":){6})", IPV6LS32, r")|",
    r"(?:(?:",                                       r"::(?:", IPV6H16, r":){5})", IPV6LS32, r")|",
    r"(?:(?:(?:",                         IPV6H16, r")?::(?:", IPV6H16, r":){4})", IPV6LS32, r")|",
    r"(?:(?:(?:(?:", IPV6H16, r":){0,1}", IPV6H16, r")?::(?:", IPV6H16, r":){3})", IPV6LS32, r")|",
    r"(?:(?:(?:(?:", IPV6H16, r":){0,2}", IPV6H16, r")?::(?:", IPV6H16, r":){2})", IPV6LS32, r")|",
    r"(?:(?:(?:(?:", IPV6H16, r":){0,3}", IPV6H16, r")?::(?:", IPV6H16, r":){1})", IPV6LS32, r")|",
    r"(?:(?:(?:(?:", IPV6H16, r":){0,4}", IPV6H16, r")?::",                   ")", IPV6LS32, r")|",
    r"(?:(?:(?:(?:", IPV6H16, r":){0,5}", IPV6H16, r")?::",                   ")", IPV6H16,  r")|",
    r"(?:(?:(?:(?:", IPV6H16, r":){0,6}", IPV6H16, r")?::",                                  ")))");

/// Returns the regular expression string to match IP addresses (v4 or v6)
#[rustfmt::skip]
pub const IPRE_STR: &str = concatcp!(r"(?:", IPV4RE_STR, "|", IPV6RE_STR, ")");

/// Regular expression string to match IP addresses where IPv6 addresses require brackets around
/// them, while for IPv4 they are forbidden.
#[rustfmt::skip]
pub const IPRE_BRACKET_STR: &str = concatcp!(r"(?:", IPV4RE_STR, r"|\[(?:", IPV6RE_STR, r")\]", r")");

#[rustfmt::skip]
pub const CIDR_V4_REGEX_STR: &str = concatcp!(r"(?:", IPV4RE_STR, r"/\d{1,2})$");

#[rustfmt::skip]
pub const CIDR_V6_REGEX_STR: &str = concatcp!(r"(?:", IPV6RE_STR, r"/\d{1,3})$");

#[rustfmt::skip]
const SAFE_ID_REGEX_STR: &str = r"(?:[A-Za-z0-9_][A-Za-z0-9._\-]*)";

const_regex! {
    /// IPv4 regular expression.
    pub IP_V4_REGEX = concatcp!(r"^", IPV4RE_STR, r"$");

    /// IPv6 regular expression.
    pub IP_V6_REGEX = concatcp!(r"^", IPV6RE_STR, r"$");

    /// Regex to match IP addresses (V4 or V6)
    pub IP_REGEX = concatcp!(r"^", IPRE_STR, r"$");

    /// Regex to match IP addresses where IPv6 addresses require brackets around
    /// them, while for IPv4 they are forbidden.
    pub IP_BRACKET_REGEX = concatcp!(r"^", IPRE_BRACKET_STR, r"$");

    pub CIDR_V4_REGEX = concatcp!(r"^", CIDR_V4_REGEX_STR, r"$");
    pub CIDR_V6_REGEX = concatcp!(r"^", CIDR_V6_REGEX_STR, r"$");
    pub CIDR_REGEX = concatcp!(r"^(?:", CIDR_V4_REGEX_STR, "|",  CIDR_V6_REGEX_STR, r")$");

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

#[test]
fn test_regexes() {
    assert!(IP_REGEX.is_match("127.0.0.1"));
    assert!(IP_V4_REGEX.is_match("127.0.0.1"));
    assert!(!IP_V6_REGEX.is_match("127.0.0.1"));

    assert!(CIDR_V4_REGEX.is_match("127.0.0.1/24"));
    assert!(CIDR_REGEX.is_match("127.0.0.1/24"));

    assert!(IP_REGEX.is_match("::1"));
    assert!(IP_REGEX.is_match("2014:b3a::27"));
    assert!(IP_REGEX.is_match("2014:b3a::192.168.0.1"));
    assert!(IP_REGEX.is_match("2014:b3a:0102:adf1:1234:4321:4afA:BCDF"));
    assert!(!IP_V4_REGEX.is_match("2014:b3a:0102:adf1:1234:4321:4afA:BCDF"));
    assert!(IP_V6_REGEX.is_match("2014:b3a:0102:adf1:1234:4321:4afA:BCDF"));

    assert!(CIDR_V6_REGEX.is_match("2014:b3a:0102:adf1:1234:4321:4afA:BCDF/60"));
    assert!(CIDR_REGEX.is_match("2014:b3a:0102:adf1:1234:4321:4afA:BCDF/60"));

    assert!(IP_BRACKET_REGEX.is_match("127.0.0.1"));
    assert!(IP_BRACKET_REGEX.is_match("[::1]"));
    assert!(IP_BRACKET_REGEX.is_match("[2014:b3a::27]"));
    assert!(IP_BRACKET_REGEX.is_match("[2014:b3a::192.168.0.1]"));
    assert!(IP_BRACKET_REGEX.is_match("[2014:b3a:0102:adf1:1234:4321:4afA:BCDF]"));
}
