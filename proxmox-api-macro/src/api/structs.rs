use std::convert::TryInto;

use failure::Error;

use proc_macro2::{Ident, TokenStream};
use quote::quote_spanned;

use super::Schema;
use crate::util::{self, JSONObject};

pub fn handle_struct(attribs: JSONObject, mut stru: syn::ItemStruct) -> Result<TokenStream, Error> {
    let mut schema: Schema = attribs.try_into()?;

    if schema.description.is_none() {
        let (doc_comment, doc_span) = util::get_doc_comments(&stru.attrs)?;
        util::derive_descriptions(&mut schema, &mut None, &doc_comment, doc_span)?;
    }

    match &stru.fields {
        // unit structs, not sure about these?
        syn::Fields::Unit => finish_schema(schema, &stru, &stru.ident),
        syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            handle_newtype_struct(schema, &mut stru)
        }
        syn::Fields::Unnamed(fields) => bail!(
            fields.paren_token.span,
            "api macro does not support tuple structs"
        ),
        syn::Fields::Named(fields) => handle_regular_struct(schema, &mut stru),
    }
}

pub fn finish_schema(
    schema: Schema,
    stru: &syn::ItemStruct,
    name: &Ident,
) -> Result<TokenStream, Error> {
    let schema = {
        let mut ts = TokenStream::new();
        schema.to_schema(&mut ts)?;
        ts
    };

    Ok(quote_spanned! { name.span() =>
        #stru
        impl #name {
            pub const API_SCHEMA: &'static ::proxmox::api::schema::Schema = #schema;
        }
    })
}

pub fn handle_newtype_struct(
    schema: Schema,
    stru: &mut syn::ItemStruct,
) -> Result<TokenStream, Error> {
    finish_schema(schema, &stru, &stru.ident)
}

pub fn handle_regular_struct(
    schema: Schema,
    stru: &mut syn::ItemStruct,
) -> Result<TokenStream, Error> {
    todo!();
}
