use std::convert::TryInto;

use failure::Error;

use proc_macro2::TokenStream;
use quote::quote_spanned;

use super::Schema;
use crate::util::JSONObject;

/// Parse `input`, `returns` and `protected` attributes out of an function annotated
/// with an `#[api]` attribute and produce a `const ApiMethod` named after the function.
///
/// See the top level macro documentation for a complete example.
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
