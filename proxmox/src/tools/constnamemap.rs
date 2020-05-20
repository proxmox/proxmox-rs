/// A macro to generate a list of pub const variabales and
/// an accompaning static array of a given name + value
/// (with doc comments)
///
/// Example:
/// ```
/// # use proxmox::constnamemap;
///
/// constnamemap! {
///     /// A list of privileges
///     PRIVS: u64 => {
///         /// Some comment for Priv1
///         PRIV1("Priv1") = 1;
///         PRIV2("Priv2") = 2;
///     }
/// }
/// ```
///
/// this will generate the following variables:
/// ```
/// /// Some comment for Priv1
/// pub const PRIV1: u64 = 1;
/// pub const PRIV2: u64 = 2;
///
/// /// A list of privileges
/// pub const PRIVS: &[(&str, u64)] = &[
///     ("Priv1", 1),
///     ("Priv2", 2),
/// ];
/// ```
#[macro_export(local_inner_macros)]
macro_rules! constnamemap {
    (
        $(#[$outer:meta])*
        $name:ident : $type:ty => {
            $($content:tt)+
        }
    ) => {
        __constnamemap_consts! {
            $type => $($content)+
        }

        $(#[$outer])*
        pub const $name: &[(&str, $type)] =
        __constnamemap_entries! {
            $($content)+
        };
    }
}

#[doc(hidden)]
#[macro_export(local_inner_macros)]
macro_rules! __constnamemap_consts {
    (
        $type:ty =>
        $(
            $(#[$outer:meta])*
            $name:ident($text:expr) = $value:expr;
        )+
    ) => {
        $(
            $(#[$outer])*
            pub const $name: $type = $value;
        )+
    }
}

#[doc(hidden)]
#[macro_export(local_inner_macros)]
macro_rules! __constnamemap_entries {
    (
        $(
            $(#[$outer:meta])*
            $name:ident($text:expr) = $value:expr;
        )*
    ) => {
        &[
            $(($text,$value),)*
        ]
    }
}
