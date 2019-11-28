use std::convert::TryInto;

use failure::Error;

use proc_macro2::TokenStream;
use quote::quote_spanned;

use super::Schema;
use crate::util::JSONObject;

pub fn handle_struct(attribs: JSONObject, stru: syn::ItemStruct) -> Result<TokenStream, Error> {
    let schema = {
        let schema: Schema = attribs.try_into()?;
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
