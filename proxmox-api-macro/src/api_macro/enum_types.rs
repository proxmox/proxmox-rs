//! `#[api]` handler for enums.
//!
//! Simple enums without data are string types. Note that we usually use lower case enum values,
//! but rust wants CamelCase, so unless otherwise requested by the user, we convert `CamelCase` to
//! `underscore_case` automatically.
//!
//! For "string" enums we automatically implement `ToString`, `FromStr`, and derive `Serialize` and
//! `Deserialize` via `serde_plain`.

use std::mem;

use proc_macro2::{Ident, Span, TokenStream};

use failure::Error;
use quote::quote_spanned;
use syn::spanned::Spanned;

use crate::api_def::{CommonTypeDefinition, ParameterDefinition};
use crate::parsing::Object;

use crate::util;

fn filter_api_items<F>(attrs: &mut Vec<syn::Attribute>, mut func: F) -> Result<(), Error>
where
    F: FnMut(util::ApiItem) -> Result<(), Error>,
{
    let cap = attrs.len();
    for attr in mem::replace(attrs, Vec::with_capacity(cap)) {
        if attr.path.is_ident(Ident::new("api", Span::call_site())) {
            let attrs: util::ApiAttr = syn::parse2(attr.tts)?;

            for attr in attrs.items {
                func(attr)?;
            }
        } else {
            attrs.push(attr);
        }
    }

    Ok(())
}

pub fn handle_enum(mut definition: Object, item: &mut syn::ItemEnum) -> Result<TokenStream, Error> {
    if item.generics.lt_token.is_some() {
        c_bail!(
            item.generics.span(),
            "generic types are currently not supported"
        );
    }

    let enum_ident = &item.ident;
    let enum_name = enum_ident.to_string();
    let expected = format!("valid {}", enum_ident);

    let mut has_fields = false;
    let mut has_verifier_unit_case = false;
    let mut display_entries = TokenStream::new();
    let mut from_str_entries = TokenStream::new();
    let mut verify_entries = TokenStream::new();

    for variant in item.variants.iter_mut() {
        let variant_ident = &variant.ident;
        let span = variant_ident.span();
        let underscore_name = util::to_underscore_case(&variant_ident.to_string());
        let mut underscore_name = syn::LitStr::new(&underscore_name, variant_ident.span());

        filter_api_items(&mut variant.attrs, |attr| {
            use util::ApiItem;
            match attr {
                ApiItem::Rename(to) => underscore_name = to,
                //other => c_bail!(other.span(), "unsupported attribute on enum variant"),
            }
            Ok(())
        })?;

        match &variant.fields {
            syn::Fields::Unit => {
                if !has_fields {
                    display_entries.extend(quote_spanned! {
                        span => #enum_ident::#variant_ident => write!(f, #underscore_name),
                    });

                    from_str_entries.extend(quote_spanned! {
                        span => #underscore_name => Ok(#enum_ident::#variant_ident),
                    });
                }

                if !has_verifier_unit_case {
                    has_verifier_unit_case = true;
                    verify_entries.extend(quote_spanned! { span => _ => Ok(()), });
                }
            }
            syn::Fields::Named(_) => {
                c_bail!(variant.span(), "#[api] enums cannot have struct fields");
            }
            syn::Fields::Unnamed(unnamedfields) => {
                has_fields = true;
                let unnamed = &unnamedfields.unnamed;

                if unnamed.len() != 1 {
                    c_bail!(
                        unnamed.span(),
                        "#[api] enums variants may have at most 1 element"
                    );
                }

                verify_entries.extend(quote_spanned! { unnamed.span() =>
                    #enum_ident::#variant_ident(ref inner) => {
                        ::proxmox::api::ApiType::verify(inner)
                    }
                });
            }
        }
    }

    let common = CommonTypeDefinition::from_object(&mut definition)?;
    let apidef = ParameterDefinition::from_object(definition)?;

    if let Some(validate) = apidef.validate {
        c_bail!(validate => "validators are not allowed on enum types");
    }

    let display_fromstr_impls = if has_fields {
        None
    } else {
        Some(quote_spanned! { item.span() =>
            impl ::std::fmt::Display for #enum_ident {
                fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                    match self {
                        #display_entries
                    }
                }
            }

            impl ::std::str::FromStr for #enum_ident {
                type Err = ::failure::Error;

                fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
                    match s {
                        #from_str_entries
                        _ => ::failure::bail!("expected {}", #expected),
                    }
                }
            }
        })
    };

    let verify_impl = if has_fields {
        quote_spanned! { item.span() =>
            fn verify(&self) -> ::std::result::Result<(), ::failure::Error> {
                match self {
                    #verify_entries
                }
            }
        }
    } else {
        quote_spanned! { item.span() =>
            fn verify(&self) -> ::std::result::Result<(), ::failure::Error> {
                Ok(())
            }
        }
    };

    let description = common.description;
    let parse_cli = common.cli.quote(&enum_ident);
    Ok(quote_spanned! { item.span() =>
        #display_fromstr_impls

        ::serde_plain::derive_deserialize_from_str!(#enum_ident, #expected);
        ::serde_plain::derive_serialize_from_display!(#enum_ident);
        ::proxmox::api::derive_parse_cli_from_str!(#enum_ident);

        impl ::proxmox::api::ApiType for #enum_ident {
            fn type_info() -> &'static ::proxmox::api::TypeInfo {
                const INFO: ::proxmox::api::TypeInfo = ::proxmox::api::TypeInfo {
                    name: #enum_name,
                    description: #description,
                    complete_fn: None, // FIXME!
                    parse_cli: #parse_cli,
                };
                &INFO
            }

            #verify_impl
        }
    })
}
