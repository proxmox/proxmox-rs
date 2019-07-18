use proc_macro2::Ident;

use syn::Token;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;

macro_rules! c_format_err {
    ($span:expr => $($msg:tt)*) => { syn::Error::new_spanned($span, format!($($msg)*)) };
    ($span:expr, $($msg:tt)*) => { syn::Error::new($span, format!($($msg)*)) };
}

macro_rules! c_bail {
    ($span:expr => $($msg:tt)*) => { return Err(c_format_err!($span => $($msg)*).into()) };
    ($span:expr, $($msg:tt)*) => { return Err(c_format_err!($span, $($msg)*).into()) };
}

/// Convert `this_kind_of_text` to `ThisKindOfText`.
pub fn to_camel_case(text: &str) -> String {
    let mut out = String::new();

    let mut capitalize = true;
    for c in text.chars() {
        if c == '_' {
            capitalize = true;
        } else {
            if capitalize {
                out.extend(c.to_uppercase());
                capitalize = false;
            } else {
                out.push(c);
            }
        }
    }

    out
}

/// Convert `ThisKindOfText` to `this_kind_of_text`.
pub fn to_underscore_case(text: &str) -> String {
    let mut out = String::new();

    for c in text.chars() {
        if c.is_uppercase() {
            if !out.is_empty() {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }

    out
}
