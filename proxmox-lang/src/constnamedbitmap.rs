/// A macro to generate a list of pub const variabales, and
/// an accompaning static array of a given name, the values are automatically
/// assigned to a bit (with doc comments)
///
/// Example:
/// ```
/// # use proxmox_lang::constnamedbitmap;
///
/// constnamedbitmap! {
///     /// A list of privileges
///     PRIVS: u64 => {
///         /// Some comment for Priv1
///         PRIV1("Priv1");
///         PRIV2("Priv2");
///         PRIV3("Priv3");
///     }
/// }
/// # assert!(PRIV1 == 1<<0);
/// # assert!(PRIV2 == 1<<1);
/// # assert!(PRIV3 == 1<<2);
/// ```
///
/// this will generate the following variables:
/// ```
/// /// Some comment for Priv1
/// pub const PRIV1: u64 = 1;
/// pub const PRIV2: u64 = 2;
/// pub const PRIV3: u64 = 4;
///
/// /// A list of privileges
/// pub const PRIVS: &[(&str, u64)] = &[
///     ("Priv1", PRIV1),
///     ("Priv2", PRIV2),
///     ("Priv3", PRIV3),
/// ];
/// ```
#[macro_export(local_inner_macros)]
macro_rules! constnamedbitmap {
    (
        $(#[$outer:meta])*
        $name:ident : $type:ty => {
            $($content:tt)+
        }
    ) => {
        __constnamemap_consts! {
            ($type) (0) => $($content)+
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
    (($type:ty) ($counter:expr) => ) => {};
    (
        ($type:ty) ($counter:expr) =>
        $(#[$outer:meta])*
        $name:ident($text:expr);
        $(
            $content:tt
        )*
    ) => {
        $(#[$outer])*
        pub const $name: $type = 1 << ($counter);
        __constnamemap_consts! {
                ($type) (1+$counter) => $($content)*
        }
    }
}

#[doc(hidden)]
#[macro_export(local_inner_macros)]
macro_rules! __constnamemap_entries {
    (
        $(
            $(#[$outer:meta])*
            $name:ident($text:expr);
        )*
    ) => {
        &[
            $(($text,$name),)*
        ]
    }
}
