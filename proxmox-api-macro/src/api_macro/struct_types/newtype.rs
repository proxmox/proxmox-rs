//! Handler for newtype structs `struct Foo(T)`.

use std::mem;

use proc_macro2::{Ident, Span, TokenStream};

use failure::Error;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

use crate::api_def::{CommonTypeDefinition, ParameterDefinition};
use crate::parsing::Object;

use super::StructField;

pub fn handle_newtype(
    mut definition: Object,
    type_ident: &Ident,
    item: &syn::FieldsUnnamed,
    attrs: &mut Vec<syn::Attribute>,
) -> Result<TokenStream, Error> {
    let type_s = type_ident.to_string();
    let type_span = type_ident.span();
    let type_str = syn::LitStr::new(&type_s, type_span);

    let fields = &item.unnamed;
    let field = fields.first().unwrap();

    let common = CommonTypeDefinition::from_object(&mut definition)?;

    let serialize_as_string = definition
        .remove("serialize_as_string")
        .map(|e| e.expect_lit_bool_direct())
        .transpose()?
        .unwrap_or(false);

    let apidef = ParameterDefinition::from_object(definition)?;

    let impl_verify = super::struct_fields_impl_verify(
        item.span(),
        &[StructField {
            def: apidef,
            ident: None,
            access: syn::Member::Unnamed(syn::Index {
                index: 0,
                span: type_ident.span(),
            }),
            mem_id: 0,
            string: "0".to_string(),
            strlit: syn::LitStr::new("0", type_ident.span()),
            ty: &field.ty,
        }],
    )?;

    let (impl_serialize, impl_deserialize) = if serialize_as_string {
        let expected = format!("valid {}", type_ident);
        (
            quote_spanned! { item.span() =>
                ::serde_plain::derive_serialize_from_display!(#type_ident);
            },
            quote_spanned! { item.span() =>
                ::serde_plain::derive_deserialize_from_str!(#type_ident, #expected);
            },
        )
    } else {
        (
            newtype_derive_serialize(item.span(), type_ident),
            newtype_derive_deserialize(item.span(), type_ident),
        )
    };

    let derive_impls = newtype_filter_derive_attrs(type_ident, &field.ty, attrs)?;

    let description = common.description;
    let parse_cli = common.cli.quote(&type_ident);
    Ok(quote! {
        #impl_serialize

        #impl_deserialize

        #derive_impls

        impl ::proxmox::api::ApiType for #type_ident {
            fn type_info() -> &'static ::proxmox::api::TypeInfo {
                use ::proxmox::api::cli::ParseCli;
                use ::proxmox::api::cli::ParseCliFromStr;
                const INFO: ::proxmox::api::TypeInfo = ::proxmox::api::TypeInfo {
                    name: #type_str,
                    description: #description,
                    complete_fn: None, // FIXME!
                    parse_cli: #parse_cli,
                };
                &INFO
            }

            #impl_verify
        }
    })
}

fn newtype_derive_serialize(span: Span, type_ident: &Ident) -> TokenStream {
    quote_spanned! { span =>
        impl ::serde::ser::Serialize for #type_ident {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: ::serde::ser::Serializer,
            {
                ::serde::ser::Serialize::serialize::<S>(&self.0, serializer)
            }
        }
    }
}

fn newtype_derive_deserialize(span: Span, type_ident: &Ident) -> TokenStream {
    quote_spanned! { span =>
        impl<'de> ::serde::de::Deserialize<'de> for #type_ident {
            fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
            where
                D: ::serde::de::Deserializer<'de>,
            {
                Ok(Self(::serde::de::Deserialize::<'de>::deserialize::<D>(deserializer)?))
            }
        }
    }
}

fn newtype_filter_derive_attrs(
    type_ident: &Ident,
    inner_type: &syn::Type,
    attrs: &mut Vec<syn::Attribute>,
) -> Result<TokenStream, Error> {
    let mut code = TokenStream::new();
    let mut had_from_str = false;
    let mut had_display = false;

    let cap = attrs.len();
    for mut attr in mem::replace(attrs, Vec::with_capacity(cap)) {
        if !attr.path.is_ident("derive") {
            attrs.push(attr);
            continue;
        }

        let mut content: syn::Expr = syn::parse2(attr.tokens)?;
        if let syn::Expr::Tuple(ref mut exprtuple) = content {
            for ty in mem::replace(&mut exprtuple.elems, syn::punctuated::Punctuated::new()) {
                if let syn::Expr::Path(ref exprpath) = ty {
                    if exprpath.path.is_ident("FromStr") {
                        if !had_from_str {
                            code.extend(newtype_derive_from_str(
                                exprpath.path.span(),
                                type_ident,
                                inner_type,
                            ));
                        }
                        had_from_str = true;
                        continue;
                    } else if exprpath.path.is_ident("Display") {
                        if !had_display {
                            code.extend(newtype_derive_display(exprpath.path.span(), type_ident));
                        }
                        had_display = true;
                        continue;
                    }
                }
                exprtuple.elems.push(ty);
            }
        }
        attr.tokens = quote! { #content };
        attrs.push(attr);
    }

    Ok(code)
}

fn newtype_derive_from_str(span: Span, type_ident: &Ident, inner_type: &syn::Type) -> TokenStream {
    quote_spanned! { span =>
        impl ::std::str::FromStr for #type_ident {
            type Err = <#inner_type as ::std::str::FromStr>::Err;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(::std::str::FromStr::from_str(s)?))
            }
        }
    }
}

fn newtype_derive_display(span: Span, type_ident: &Ident) -> TokenStream {
    quote_spanned! { span =>
        impl ::std::fmt::Display for #type_ident {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }
    }
}
