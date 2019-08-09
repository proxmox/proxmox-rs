//! Handler for unnamed struct types `struct Foo(T1, T2, ...)`.
//!
//! Note that single-type structs are handled in the `newtype` module instead.

use proc_macro2::{Ident, TokenStream};

use failure::{bail, Error};
use quote::quote;

use crate::api_def::{CommonTypeDefinition, ParameterDefinition};
use crate::parsing::Object;

//use super::StructField;

pub fn handle_struct_unnamed(
    mut definition: Object,
    name: &Ident,
    item: &syn::FieldsUnnamed,
) -> Result<TokenStream, Error> {
    let fields = &item.unnamed;
    if fields.len() != 1 {
        bail!("only 1 unnamed field is currently allowed for api types");
    }

    //let field = fields.first().unwrap().value();

    let common = CommonTypeDefinition::from_object(&mut definition)?;
    let apidef = ParameterDefinition::from_object(definition)?;

    let validator = match apidef.validate {
        Some(ident) => quote! { #ident(&self.0) },
        None => quote! { ::proxmox::api::ApiType::verify(&self.0) },
    };

    let description = common.description;
    let parse_cli = common.cli.quote(&name);
    Ok(quote! {
        impl ::proxmox::api::ApiType for #name {
            fn type_info() -> &'static ::proxmox::api::TypeInfo {
                use ::proxmox::api::cli::ParseCli;
                use ::proxmox::api::cli::ParseCliFromStr;
                const INFO: ::proxmox::api::TypeInfo = ::proxmox::api::TypeInfo {
                    name: stringify!(#name),
                    description: #description,
                    complete_fn: None, // FIXME!
                    parse_cli: #parse_cli,
                };
                &INFO
            }

            fn verify(&self) -> ::std::result::Result<(), ::failure::Error> {
                #validator
            }
        }
    })
}
