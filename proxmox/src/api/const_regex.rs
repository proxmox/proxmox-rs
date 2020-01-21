use std::fmt;

/// Helper to represent const regular expressions
///
/// The current Regex::new() function is not `const_fn`. Unless that
/// works, we use `ConstRegexPattern` to represent static regular
/// expressions. Please use the `const_regex` macro to generate
/// instances of this type (uses lazy_static).
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
/// # use proxmox::const_regex;
/// #
/// const_regex!{
///    FILE_EXTENSION_REGEX = r".*\.([a-zA-Z]+)$";
///    pub SHA256_HEX_REGEX = r"^[a-f0-9]{64}$";
/// }
/// ```
#[macro_export]
macro_rules! const_regex {
    () =>   {};
    ($(#[$attr:meta])* pub ($($vis:tt)+) $name:ident = $regex:expr; $($rest:tt)*) =>  {
        $crate::const_regex! { (pub ($($vis)+)) $(#[$attr])* $name = $regex; $($rest)* }
    };
    ($(#[$attr:meta])* pub $name:ident = $regex:expr; $($rest:tt)*) =>  {
        $crate::const_regex! { (pub) $(#[$attr])* $name = $regex; $($rest)* }
    };
    ($(#[$attr:meta])* $name:ident = $regex:expr; $($rest:tt)*) =>  {
        $crate::const_regex! { () $(#[$attr])* $name = $regex; $($rest)* }
    };
    (
        ($($pub:tt)*) $(#[$attr:meta])* $name:ident = $regex:expr;
        $($rest:tt)*
    ) =>  {
        $(#[$attr])* $($pub)* const $name: $crate::api::const_regex::ConstRegexPattern =
            $crate::api::const_regex::ConstRegexPattern {
                regex_string: $regex,
                regex_obj: (|| ->   &'static ::regex::Regex {
                    ::lazy_static::lazy_static! {
                        static ref SCHEMA: ::regex::Regex = ::regex::Regex::new($regex).unwrap();
                    }
                    &SCHEMA
                })
            };

        $crate::const_regex! { $($rest)* }
    };
}

#[cfg(feature = "test-harness")]
impl Eq for ConstRegexPattern {}

#[cfg(feature = "test-harness")]
impl PartialEq for ConstRegexPattern {
    fn eq(&self, rhs: &Self) -> bool {
        self.regex_string == rhs.regex_string
    }
}
