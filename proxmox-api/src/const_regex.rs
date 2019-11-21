/// Macro to generate a ConstRegexPattern
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
        $(#[$attr])* $($pub)* const $name: $crate::schema::ConstRegexPattern =
            $crate::schema::ConstRegexPattern {
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
