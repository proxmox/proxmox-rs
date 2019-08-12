use proc_macro2::Ident;

use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{parenthesized, Token};

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

pub struct ApiAttr {
    pub paren_token: syn::token::Paren,
    pub items: Punctuated<ApiItem, Token![,]>,
}

impl Parse for ApiAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(ApiAttr {
            paren_token: parenthesized!(content in input),
            items: content.parse_terminated(ApiItem::parse)?,
        })
    }
}

pub enum ApiItem {
    Rename(syn::LitStr),
}

impl Parse for ApiItem {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let what: Ident = input.parse()?;
        let what_str = what.to_string();
        match what_str.as_str() {
            "rename" => {
                let _: Token![=] = input.parse()?;
                Ok(ApiItem::Rename(input.parse()?))
            }
            _ => c_bail!(what => "unrecognized api attribute: {}", what_str),
        }
    }
}
