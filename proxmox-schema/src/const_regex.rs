use std::fmt;

/// Helper to represent const regular expressions
///
/// The current Regex::new() function is not `const_fn`. Unless that
/// works, we use `ConstRegexPattern` to represent static regular
/// expressions. Please use the `const_regex` macro to generate
/// instances of this type.
pub struct ConstRegexPattern {
    /// This is only used for documentation and debugging
    pub regex_string: &'static str,
    /// This function return the the actual Regex
    pub regex_obj: fn() -> &'static regex::Regex,
}

impl fmt::Debug for ConstRegexPattern {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.regex_string)
    }
}

impl std::ops::Deref for ConstRegexPattern {
    type Target = regex::Regex;

    fn deref(&self) -> &Self::Target {
        (self.regex_obj)()
    }
}

/// Macro to generate a ConstRegexPattern
///
/// ```
/// use proxmox_schema::const_regex;
///
/// const_regex!{
///    FILE_EXTENSION_REGEX = r".*\.([a-zA-Z]+)$";
///    pub SHA256_HEX_REGEX = r"^[a-f0-9]{64}$";
/// }
/// ```
#[macro_export]
macro_rules! const_regex {
    ($(
        $(#[$attr:meta])*
        $vis:vis $name:ident = $regex:expr;
    )+) =>  { $(
        $(#[$attr])* $vis const $name: $crate::ConstRegexPattern =
            $crate::ConstRegexPattern {
                regex_string: $regex,
                regex_obj: (|| ->   &'static ::regex::Regex {
                    static SCHEMA: std::sync::LazyLock<::regex::Regex> = std::sync::LazyLock::new(|| ::regex::Regex::new($regex).unwrap());
                    &SCHEMA
                })
            };
    )+ };
}

#[cfg(feature = "test-harness")]
impl Eq for ConstRegexPattern {}

#[cfg(feature = "test-harness")]
impl PartialEq for ConstRegexPattern {
    fn eq(&self, rhs: &Self) -> bool {
        self.regex_string == rhs.regex_string
    }
}
