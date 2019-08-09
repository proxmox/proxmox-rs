//! Module for struct handling.
//!
//! This will forward to specialized variants for named structs, tuple structs and newtypes.

use proc_macro2::{Ident, Span, TokenStream};

use failure::Error;
use quote::quote_spanned;
use syn::spanned::Spanned;

use crate::api_def::ParameterDefinition;
use crate::parsing::Object;

mod named;
mod newtype;
mod unnamed;

/// Commonly used items of a struct field.
pub struct StructField<'i, 't> {
    def: ParameterDefinition,
    ident: Option<&'i Ident>,
    access: syn::Member,
    mem_id: isize,
    string: String,
    strlit: syn::LitStr,
    ty: &'t syn::Type,
}

pub fn handle_struct(definition: Object, item: &mut syn::ItemStruct) -> Result<TokenStream, Error> {
    if item.generics.lt_token.is_some() {
        c_bail!(
            item.generics.span(),
            "generic types are currently not supported"
        );
    }

    let name = &item.ident;

    match item.fields {
        syn::Fields::Unit => c_bail!(item.span(), "unit types are not allowed"),
        syn::Fields::Unnamed(ref fields) if fields.unnamed.len() == 1 => {
            newtype::handle_newtype(definition, name, fields, &mut item.attrs)
        }
        syn::Fields::Unnamed(ref fields) => {
            unnamed::handle_struct_unnamed(definition, name, fields)
        }
        syn::Fields::Named(ref fields) => named::handle_struct_named(definition, name, fields),
    }
}

fn struct_fields_impl_verify(span: Span, fields: &[StructField]) -> Result<TokenStream, Error> {
    let mut body = TokenStream::new();
    for field in fields {
        let field_access = &field.access;
        let field_str = &field.strlit;

        // first of all, recurse into the contained types:
        body.extend(quote_spanned! { field_access.span() =>
            ::proxmox::api::ApiType::verify(&self.#field_access)?;
        });

        // then go through all the additional verifiers:

        if let Some(ref value) = field.def.minimum {
            body.extend(quote_spanned! { value.span() =>
                let value = #value;
                if !::proxmox::api::verify::TestMinMax::test_minimum(&self.#field_access, &value) {
                    error_list.push(
                        format!("field {} out of range, must be >= {}", #field_str, value)
                    );
                }
            });
        }

        if let Some(ref value) = field.def.maximum {
            body.extend(quote_spanned! { value.span() =>
                let value = #value;
                if !::proxmox::api::verify::TestMinMax::test_maximum(&self.#field_access, &value) {
                    error_list.push(
                        format!("field {} out of range, must be <= {}", #field_str, value)
                    );
                }
            });
        }

        if let Some(ref value) = field.def.minimum_length {
            body.extend(quote_spanned! { value.span() =>
                let value = #value;
                if !::proxmox::api::verify::TestMinMaxLen::test_minimum_length(
                    &self.#field_access,
                    value,
                ) {
                    error_list.push(
                        format!("field {} too short, must be >= {} characters", #field_str, value)
                    );
                }
            });
        }

        if let Some(ref value) = field.def.maximum_length {
            body.extend(quote_spanned! { value.span() =>
                let value = #value;
                if !::proxmox::api::verify::TestMinMaxLen::test_maximum_length(
                    &self.#field_access,
                    value,
                ) {
                    error_list.push(
                        format!("field {} too long, must be <= {} characters", #field_str, value)
                    );
                }
            });
        }

        if let Some(ref value) = field.def.format {
            body.extend(quote_spanned! { value.span() =>
                if !#value::verify(&self.#field_access) {
                    error_list.push(
                        format!("field {} does not match format {}", #field_str, #value::NAME)
                    );
                }
            });
        }

        if let Some(ref value) = field.def.pattern {
            match value {
                syn::Expr::Lit(regex) => body.extend(quote_spanned! { value.span() =>
                    {
                        ::lazy_static::lazy_static! {
                            static ref RE: ::regex::Regex = ::regex::Regex::new(#regex).unwrap();
                        }
                        if !RE.is_match(&self.#field_access) {
                            error_list.push(format!(
                                "field {} does not match the allowed pattern: {}",
                                #field_str,
                                #regex,
                            ));
                        }
                    }
                }),
                regex => body.extend(quote_spanned! { value.span() =>
                    if !#regex.is_match(&self.#field_access) {
                        error_list.push(
                            format!("field {} does not match the allowed pattern", #field_str)
                        );
                    }
                }),
            }
        }

        if let Some(ref value) = field.def.validate {
            body.extend(quote_spanned! { value.span() =>
                if let Err(err) = #value(&self.#field_access) {
                    error_list.push(err.to_string());
                }
            });
        }
    }

    if !body.is_empty() {
        body = quote_spanned! { span =>
            #[allow(unused_mut)]
            let mut error_list: Vec<String> = Vec::new();
            #body
            if !error_list.is_empty() {
                let mut error_string = String::new();
                for e in error_list.iter() {
                    if !error_string.is_empty() {
                        error_string.push_str("\n");
                    }
                    error_string.push_str(&e);
                }
                return Err(::failure::format_err!("{}", error_string));
            }
        };
    }

    Ok(quote_spanned! { span =>
        fn verify(&self) -> ::std::result::Result<(), ::failure::Error> {
            #body

            Ok(())
        }
    })
}
