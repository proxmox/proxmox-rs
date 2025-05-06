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
#[macro_export]
macro_rules! constnamedbitmap {
    (
        $(#[$doc:meta])*
        $name:ident : $type:ty => {
            $(
                $(#[$item_doc:meta])*
                $item_name:ident($item_text:expr);
            )+
        }
    ) => {
        $crate::constnamedbitmap!(
            const { 1 }
            $(
                $(#[$item_doc])*
                $item_name($item_text);
            )*
        );

        $(#[$doc])*
        pub const $name: &[(&str, $type)] = &[
            $( ($item_text, $item_name), )+
        ];
    };
    (const { $value:expr }) => ();
    (const { $value:expr }
        $(#[$item_doc:meta])*
        $item_name:ident($item_text:expr);

        $($rest:tt)*
    ) => (
        $(#[$item_doc])*
        pub const $item_name: u64 = $value;
        $crate::constnamedbitmap!(const {$item_name << 1} $($rest)*);
    );
}
