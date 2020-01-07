use std::convert::TryInto;

use failure::Error;

use proc_macro2::TokenStream;
use quote::quote_spanned;

use super::Schema;
use crate::util::{self, JSONObject};

pub fn handle_struct(attribs: JSONObject, stru: syn::ItemStruct) -> Result<TokenStream, Error> {
    let mut schema: Schema = attribs.try_into()?;

    if schema.description.is_none() {
        let (doc_comment, doc_span) = util::get_doc_comments(&stru.attrs)?;
        util::derive_descriptions(&mut schema, &mut None, &doc_comment, doc_span)?;
    }

    let schema = {
        let mut ts = TokenStream::new();
        schema.to_schema(&mut ts)?;
        ts
    };

    let name = &stru.ident;

    Ok(quote_spanned! { name.span() =>
        #stru
        impl #name {
            pub const API_SCHEMA: &'static ::proxmox::api::schema::Schema = #schema;
        }
    })
}
