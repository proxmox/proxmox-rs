/// Helper macro to generate a simple string type wrapper.
///
/// This is meant to be used with an API-type tuple struct containing a single `String` like this:
///
/// ```
/// # use proxmox_schema::{api_string_type, ApiStringFormat};
/// # use proxmox_api_macro::api;
/// # const PROXMOX_SAFE_ID_FORMAT: ApiStringFormat = ApiStringFormat::Enum(&[]);
/// use serde::{Deserialize, Serialize};
///
/// api_string_type! {
///     #[api(format: &PROXMOX_SAFE_ID_FORMAT)]
///     /// ACME account name.
///     #[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
///     #[serde(transparent)]
///     pub struct AccountName(String);
/// }
/// ```
///
/// This will automatically implements:
/// * `Display` as a pass-through to `String`'s `Display`
/// * `Deref`
/// * `DerefMut`
/// * `AsRef<str>`
/// * `TryFrom<String>`
/// * `fn into_string(self) -> String`
/// * `fn as_str(&self) -> &str`
/// * `fn from_string(inner: String) -> Result<Self, anyhow::Error>` using
///   `StringSchema::check_constraints`.
/// * `unsafe fn from_string_unchecked(inner: String) -> Self`
#[macro_export]
macro_rules! api_string_type {
    (
        $(#[$doc:meta])*
        $vis:vis struct $name:ident(String);
    ) => (
        $(#[$doc])*
        $vis struct $name(String);

        impl ::std::ops::Deref for $name {
            type Target = str;

            #[inline]
            fn deref(&self) -> &str {
                &self.0
            }
        }

        impl ::std::ops::DerefMut for $name {
            #[inline]
            fn deref_mut(&mut self) -> &mut str {
                &mut self.0
            }
        }

        impl AsRef<str> for $name {
            #[inline]
            fn as_ref(&self) -> &str {
                self.0.as_ref()
            }
        }

        impl ::std::convert::TryFrom<String> for $name {
            type Error = ::anyhow::Error;

            fn try_from(inner: String) -> Result<Self, ::anyhow::Error> {
                Self::from_string(inner)
            }
        }

        impl $name {
            /// Get the contained string.
            pub fn into_string(self) -> String {
                self.0
            }

            /// Get the string as slice.
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }

            /// Create an instance directly from a `String`.
            ///
            /// # Safety
            ///
            /// It is the caller's job to have validated the contents.
            /// While there are no memory safety issues, a wrong string can cause API calls to
            /// fail parameter validation.
            pub unsafe fn from_string_unchecked(name: String) -> Self {
                Self(name)
            }

            /// Create an instance directly from a `String`, validating it using the API schema's
            /// [`check_constraints`](::proxmox_schema::StringSchema::check_constraints())
            /// method.
            pub fn from_string(inner: String) -> Result<Self, ::anyhow::Error> {
                use $crate::ApiType;
                match &Self::API_SCHEMA {
                    $crate::Schema::String(s) => s.check_constraints(&inner)?,
                    _ => unreachable!(),
                }
                Ok(Self(inner))
            }
        }

        impl ::std::fmt::Display for $name {
            #[inline]
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }
    );
}
