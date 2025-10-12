//! The "basic" api types we generally require along with some of their macros.
use const_format::concatcp;

use crate::{ApiStringFormat, ArraySchema, Schema, StringSchema};

#[rustfmt::skip]
const IPV4OCTET: &str = r"(?:25[0-5]|(?:2[0-4]|1[0-9]|[1-9])?[0-9])";

#[rustfmt::skip]
const IPV6H16: &str = r"(?:[0-9a-fA-F]{1,4})";

/// Regular expression string to match IPv4 addresses
#[rustfmt::skip]
pub const IPV4RE_STR: &str = concatcp!(r"(?:(?:", IPV4OCTET, r"\.){3}", IPV4OCTET, ")");

#[rustfmt::skip]
const IPV6LS32: &str = concatcp!(r"(?:(?:", IPV4RE_STR, "|", IPV6H16, ":", IPV6H16, "))" );

/// Regular expression string to match IPv6 addresses
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

/// Regular expression string to match IP addresses (v4 or v6)
#[rustfmt::skip]
pub const IPRE_STR: &str = concatcp!(r"(?:", IPV4RE_STR, "|", IPV6RE_STR, ")");

/// Regular expression string to match IP addresses where IPv6 addresses require brackets around
/// them, while for IPv4 they are forbidden.
#[rustfmt::skip]
pub const IPRE_BRACKET_STR: &str = concatcp!(r"(?:", IPV4RE_STR, r"|\[(?:", IPV6RE_STR, r")\]", r")");

/// Regular expression string to match CIDRv4 network
#[rustfmt::skip]
pub const CIDR_V4_REGEX_STR: &str = concatcp!(r"(?:", IPV4RE_STR, r"/\d{1,2})$");

/// Regular expression string to match CIDRv6 network
#[rustfmt::skip]
pub const CIDR_V6_REGEX_STR: &str = concatcp!(r"(?:", IPV6RE_STR, r"/\d{1,3})$");

/// Regular expression string for safe identifiers.
#[rustfmt::skip]
pub const SAFE_ID_REGEX_STR: &str = r"(?:[A-Za-z0-9_][A-Za-z0-9._\-]*)";

#[rustfmt::skip]
pub const DNS_LABEL_STR: &str = r"(?:[a-zA-Z0-9](?:[a-zA-Z0-9\-]*[a-zA-Z0-9])?)";

#[rustfmt::skip]
pub const DNS_NAME_STR: &str = concatcp!(r"(?:(?:", DNS_LABEL_STR, r"\.)*", DNS_LABEL_STR, ")");

#[rustfmt::skip]
pub const DNS_ALIAS_LABEL_STR: &str = r"(?:[a-zA-Z0-9_](?:[a-zA-Z0-9\-]*[a-zA-Z0-9])?)";

#[rustfmt::skip]
pub const DNS_ALIAS_NAME_STR: &str = concatcp!(r"(?:(?:", DNS_ALIAS_LABEL_STR , r"\.)*", DNS_ALIAS_LABEL_STR, ")");

#[rustfmt::skip]
pub const PORT_REGEX_STR: &str = r"(?:[0-9]{1,4}|[1-5][0-9]{4}|6[0-4][0-9]{3}|65[0-4][0-9]{2}|655[0-2][0-9]|6553[0-5])";

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
    /// contains further information why it is reasonable to restrict
    /// names this way. This is not only useful for filenames, but for
    /// any identifier command line tools work with.
    pub SAFE_ID_REGEX = concatcp!(r"^", SAFE_ID_REGEX_STR, r"$");
    /// Password. Allow everything but control characters.
    pub PASSWORD_REGEX = r"^[[:^cntrl:]]*$";
    /// Single line comment. Allow everything but control characters.
    pub SINGLE_LINE_COMMENT_REGEX = r"^[[:^cntrl:]]*$";
    /// Comment spawning multiple lines. Allow everything but control characters.
    pub MULTI_LINE_COMMENT_REGEX = r"(?m)^([[:^cntrl:]]*)$";

    pub HOSTNAME_REGEX = r"^(?:[a-zA-Z0-9](?:[a-zA-Z0-9\-]*[a-zA-Z0-9])?)$";
    pub DNS_NAME_REGEX = concatcp!(r"^", DNS_NAME_STR, r"$");
    pub DNS_ALIAS_REGEX = concatcp!(r"^", DNS_ALIAS_NAME_STR, r"$");
    pub DNS_NAME_OR_IP_REGEX = concatcp!(r"^(?:", DNS_NAME_STR, "|",  IPRE_STR, r")$");
    pub HOST_PORT_REGEX = concatcp!(r"^(?:", DNS_NAME_STR, "|", IPRE_BRACKET_STR, "):", PORT_REGEX_STR ,"$");
    pub HTTP_URL_REGEX = concatcp!(r"^https?://(?:(?:(?:", DNS_NAME_STR, "|", IPRE_BRACKET_STR, ")(?::", PORT_REGEX_STR ,")?)|", IPV6RE_STR,")(?:/[^\x00-\x1F\x7F]*)?$");

    /// Regex to match SHA256 Digest.
    pub SHA256_HEX_REGEX = r"^[a-f0-9]{64}$";

    pub FINGERPRINT_SHA256_REGEX = r"^(?:[0-9a-fA-F][0-9a-fA-F])(?::[0-9a-fA-F][0-9a-fA-F]){31}$";

    pub UUID_REGEX = r"^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$";

    /// Regex to match systemd date/time format.
    pub SYSTEMD_DATETIME_REGEX = r"^\d{4}-\d{2}-\d{2}( \d{2}:\d{2}(:\d{2})?)?$";

    /// Regex that (loosely) matches URIs according to [RFC 2396](https://www.rfc-editor.org/rfc/rfc2396.txt)
    /// This does not completely match a URI, but rather disallows all the prohibited characters
    /// specified in the RFC.
    pub GENERIC_URI_REGEX = r#"^[^\x00-\x1F\x7F <>#"]*$"#;

    pub BLOCKDEVICE_NAME_REGEX = r"^(?:(?:h|s|x?v)d[a-z]+)|(?:nvme\d+n\d+)$";
    pub BLOCKDEVICE_DISK_AND_PARTITION_NAME_REGEX = r"^(?:(?:h|s|x?v)d[a-z]+\d*)|(?:nvme\d+n\d+(p\d+)?)$";
}

pub const SAFE_ID_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&SAFE_ID_REGEX);
pub const PASSWORD_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&PASSWORD_REGEX);
pub const SINGLE_LINE_COMMENT_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&SINGLE_LINE_COMMENT_REGEX);
pub const MULTI_LINE_COMMENT_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&MULTI_LINE_COMMENT_REGEX);

pub const IP_V4_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&IP_V4_REGEX);
pub const IP_V6_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&IP_V6_REGEX);
pub const IP_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&IP_REGEX);
pub const CIDR_V4_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&CIDR_V4_REGEX);
pub const CIDR_V6_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&CIDR_V6_REGEX);
pub const CIDR_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&CIDR_REGEX);
pub const UUID_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&UUID_REGEX);
pub const BLOCKDEVICE_NAME_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&BLOCKDEVICE_NAME_REGEX);
pub const BLOCKDEVICE_DISK_AND_PARTITION_NAME_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&BLOCKDEVICE_DISK_AND_PARTITION_NAME_REGEX);

pub const SYSTEMD_DATETIME_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&SYSTEMD_DATETIME_REGEX);

pub const HOSTNAME_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&HOSTNAME_REGEX);
pub const HOST_PORT_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&HOST_PORT_REGEX);
pub const HTTP_URL_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&HTTP_URL_REGEX);

pub const DNS_ALIAS_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&DNS_ALIAS_REGEX);
pub const DNS_NAME_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&DNS_NAME_REGEX);
pub const DNS_NAME_OR_IP_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&DNS_NAME_OR_IP_REGEX);

pub const IP_V4_SCHEMA: Schema = StringSchema::new("IPv4 address.")
    .format(&IP_V4_FORMAT)
    .max_length(15)
    .schema();

pub const IP_V6_SCHEMA: Schema = StringSchema::new("IPv6 address.")
    .format(&IP_V6_FORMAT)
    .max_length(39)
    .schema();

pub const IP_SCHEMA: Schema = StringSchema::new("IP (IPv4 or IPv6) address.")
    .format(&IP_FORMAT)
    .max_length(39)
    .schema();

pub const CIDR_V4_SCHEMA: Schema = StringSchema::new("IPv4 address with netmask (CIDR notation).")
    .format(&CIDR_V4_FORMAT)
    .max_length(18)
    .schema();

pub const CIDR_V6_SCHEMA: Schema = StringSchema::new("IPv6 address with netmask (CIDR notation).")
    .format(&CIDR_V6_FORMAT)
    .max_length(43)
    .schema();

pub const CIDR_SCHEMA: Schema =
    StringSchema::new("IP address (IPv4 or IPv6) with netmask (CIDR notation).")
        .format(&CIDR_FORMAT)
        .max_length(43)
        .schema();

pub const FINGERPRINT_SHA256_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&FINGERPRINT_SHA256_REGEX);

pub const CERT_FINGERPRINT_SHA256_SCHEMA: Schema =
    StringSchema::new("X509 certificate fingerprint (sha256).")
        .format(&FINGERPRINT_SHA256_FORMAT)
        .schema();

pub const PASSWORD_SCHEMA: Schema = StringSchema::new("Password.")
    .format(&PASSWORD_FORMAT)
    .min_length(1)
    .max_length(1024)
    .schema();

pub const COMMENT_SCHEMA: Schema = StringSchema::new("Comment.")
    .format(&SINGLE_LINE_COMMENT_FORMAT)
    .max_length(128)
    .schema();

pub const MULTI_LINE_COMMENT_SCHEMA: Schema = StringSchema::new("Comment (multiple lines).")
    .format(&MULTI_LINE_COMMENT_FORMAT)
    .schema();

pub const HOSTNAME_SCHEMA: Schema = StringSchema::new("Hostname (as defined in RFC1123).")
    .format(&HOSTNAME_FORMAT)
    .schema();

pub const DNS_NAME_OR_IP_SCHEMA: Schema = StringSchema::new("DNS name or IP address.")
    .format(&DNS_NAME_OR_IP_FORMAT)
    .schema();

pub const HOST_PORT_SCHEMA: Schema =
    StringSchema::new("host:port combination (Host can be DNS name or IP address).")
        .format(&HOST_PORT_FORMAT)
        .schema();

pub const HTTP_URL_SCHEMA: Schema = StringSchema::new("HTTP(s) url with optional port.")
    .format(&HTTP_URL_FORMAT)
    .schema();

pub const NODE_SCHEMA: Schema = StringSchema::new("Node name (or 'localhost')")
    .format(&HOSTNAME_FORMAT)
    .schema();

pub const SERVICE_ID_SCHEMA: Schema = StringSchema::new("Service ID.").max_length(256).schema();

pub const TIME_ZONE_SCHEMA: Schema = StringSchema::new(
    "Time zone. The file '/usr/share/zoneinfo/zone.tab' contains the list of valid names.",
)
.format(&SINGLE_LINE_COMMENT_FORMAT)
.min_length(2)
.max_length(64)
.schema();

pub const BLOCKDEVICE_NAME_SCHEMA: Schema =
    StringSchema::new("Block device name (/sys/block/<name>).")
        .format(&BLOCKDEVICE_NAME_FORMAT)
        .min_length(3)
        .max_length(64)
        .schema();

pub const BLOCKDEVICE_DISK_AND_PARTITION_NAME_SCHEMA: Schema =
    StringSchema::new("(Partition) block device name (/sys/class/block/<name>).")
        .format(&BLOCKDEVICE_DISK_AND_PARTITION_NAME_FORMAT)
        .min_length(3)
        .max_length(64)
        .schema();

pub const DISK_ARRAY_SCHEMA: Schema =
    ArraySchema::new("Disk name list.", &BLOCKDEVICE_NAME_SCHEMA).schema();

pub const DISK_LIST_SCHEMA: Schema = StringSchema::new("A list of disk names, comma separated.")
    .format(&ApiStringFormat::PropertyString(&DISK_ARRAY_SCHEMA))
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
