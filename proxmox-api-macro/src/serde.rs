//! Serde support module.
//!
//! The `#![api]` macro needs to be able to cope with some `#[serde(...)]` attributes such as
//! `rename` and `rename_all`.

use std::convert::TryFrom;

use syn::punctuated::Punctuated;
use syn::Token;

/// Serde name types.
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenameAll {
    LowerCase,
    UpperCase,
    PascalCase,
    CamelCase,
    SnakeCase,
    ScreamingSnakeCase,
    KebabCase,
    ScreamingKebabCase,
}

impl TryFrom<&syn::Lit> for RenameAll {
    type Error = syn::Error;
    fn try_from(s: &syn::Lit) -> Result<Self, syn::Error> {
        match s {
            syn::Lit::Str(s) => Self::try_from(s),
            _ => bail!(s => "expected rename type as string"),
        }
    }
}

impl TryFrom<&syn::LitStr> for RenameAll {
    type Error = syn::Error;
    fn try_from(s: &syn::LitStr) -> Result<Self, syn::Error> {
        let s = s.value();
        if s == "lowercase" {
            Ok(RenameAll::LowerCase)
        } else if s == "UPPERCASE" {
            Ok(RenameAll::UpperCase)
        } else if s == "PascalCase" {
            Ok(RenameAll::PascalCase)
        } else if s == "camelCase" {
            Ok(RenameAll::CamelCase)
        } else if s == "snake_case" {
            Ok(RenameAll::SnakeCase)
        } else if s == "SCREAMING_SNAKE_CASE" {
            Ok(RenameAll::ScreamingSnakeCase)
        } else if s == "kebab-case" {
            Ok(RenameAll::KebabCase)
        } else if s == "SCREAMING-KEBAB-CASE" {
            Ok(RenameAll::ScreamingKebabCase)
        } else {
            bail!(&s => "unhandled `rename_all` type: {}", s.to_string())
        }
    }
}

impl RenameAll {
    /// Like in serde, we assume that fields are in `snake_case` and enum variants are in
    /// `PascalCase`, so we only perform the changes required for fields here!
    pub fn apply_to_field(&self, s: &str) -> String {
        match self {
            RenameAll::SnakeCase => s.to_owned(), // this is our source type
            RenameAll::ScreamingSnakeCase => s.to_uppercase(), // capitalized source type
            RenameAll::LowerCase => s.to_lowercase(),
            RenameAll::UpperCase => s.to_uppercase(),
            RenameAll::PascalCase => {
                // Strip underscores and capitalize instead:
                let mut out = String::new();
                let mut cap = true;
                for c in s.chars() {
                    if c == '_' {
                        cap = true;
                    } else if cap {
                        cap = false;
                        out.push(c.to_ascii_uppercase());
                    } else {
                        out.push(c.to_ascii_lowercase());
                    }
                }
                out
            }
            RenameAll::CamelCase => {
                let s = RenameAll::PascalCase.apply_to_field(s);
                s[..1].to_ascii_lowercase() + &s[1..]
            }
            RenameAll::KebabCase => s.replace('_', "-"),
            RenameAll::ScreamingKebabCase => s.replace('_', "-").to_ascii_uppercase(),
        }
    }

    /// Like in serde, we assume that fields are in `snake_case` and enum variants are in
    /// `PascalCase`, so we only perform the changes required for enum variants here!
    pub fn apply_to_variant(&self, s: &str) -> String {
        match self {
            RenameAll::PascalCase => s.to_owned(), // this is our source type
            RenameAll::CamelCase => s[..1].to_ascii_lowercase() + &s[1..],
            RenameAll::LowerCase => s.to_lowercase(),
            RenameAll::UpperCase => s.to_uppercase(),
            RenameAll::SnakeCase => {
                // Relatively simple: all lower-case, and new words get split by underscores:
                let mut out = String::new();
                for (i, c) in s.char_indices() {
                    if i > 0 && c.is_uppercase() {
                        out.push('_');
                    }
                    out.push(c.to_ascii_lowercase());
                }
                out
            }
            RenameAll::KebabCase => RenameAll::SnakeCase.apply_to_variant(s).replace('_', "-"),
            RenameAll::ScreamingSnakeCase => RenameAll::SnakeCase
                .apply_to_variant(s)
                .to_ascii_uppercase(),
            RenameAll::ScreamingKebabCase => RenameAll::KebabCase
                .apply_to_variant(s)
                .to_ascii_uppercase(),
        }
    }
}

/// `serde` container attributes we support
#[derive(Default)]
pub struct ContainerAttrib {
    pub rename_all: Option<RenameAll>,
}

impl TryFrom<&[syn::Attribute]> for ContainerAttrib {
    type Error = syn::Error;

    fn try_from(attributes: &[syn::Attribute]) -> Result<Self, syn::Error> {
        let mut this: Self = Default::default();

        for attrib in attributes {
            let list = match &attrib.meta {
                syn::Meta::List(list) if list.path.is_ident("serde") => list,
                _ => continue,
            };

            let args =
                list.parse_args_with(Punctuated::<syn::Meta, Token![,]>::parse_terminated)?;

            for arg in args {
                if let syn::Meta::NameValue(var) = arg {
                    if !var.path.is_ident("rename_all") {
                        continue;
                    }
                    match &var.value {
                        syn::Expr::Lit(lit) => {
                            let rename_all = RenameAll::try_from(&lit.lit)?;
                            if this.rename_all.is_some() && this.rename_all != Some(rename_all) {
                                error!(var.value => "multiple conflicting 'rename_all' attributes");
                            }
                            this.rename_all = Some(rename_all);
                        }
                        _ => error!(var.value => "invalid 'rename_all' value type"),
                    }
                }
            }
        }

        Ok(this)
    }
}

/// `serde` field/variant attributes we support
#[derive(Default)]
pub struct FieldAttrib {
    pub rename: Option<syn::LitStr>,
    pub flatten: bool,
}

impl FieldAttrib {
    pub fn parse_attribute(&mut self, attrib: &syn::Attribute) -> Result<(), syn::Error> {
        let list = match &attrib.meta {
            syn::Meta::List(list) if list.path.is_ident("serde") => list,
            _ => return Ok(()),
        };

        let args = list.parse_args_with(Punctuated::<syn::Meta, Token![,]>::parse_terminated)?;

        for arg in args {
            let path = arg.path();
            if path.is_ident("rename") {
                match &arg.require_name_value()?.value {
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(rename),
                        ..
                    }) => {
                        if self.rename.is_some() && self.rename.as_ref() != Some(rename) {
                            error!(&rename => "multiple conflicting 'rename' attributes");
                        }
                        self.rename = Some(rename.clone());
                    }
                    value => error!(value => "'rename' value must be a string literal"),
                }
            } else if path.is_ident("flatten") {
                arg.require_path_only()?;
                self.flatten = true;
            }
        }

        Ok(())
    }
}

impl TryFrom<&[syn::Attribute]> for FieldAttrib {
    type Error = syn::Error;

    fn try_from(attributes: &[syn::Attribute]) -> Result<Self, syn::Error> {
        let mut this: Self = Default::default();

        for attrib in attributes {
            this.parse_attribute(attrib)?;
        }

        Ok(this)
    }
}
