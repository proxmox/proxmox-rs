use proc_macro2::{Delimiter, TokenStream, TokenTree};

use failure::Error;
use quote::ToTokens;
use syn::spanned::Spanned;

use crate::parsing::parse_object;

mod enum_types;
mod function;
mod struct_types;

pub fn api_macro(attr: TokenStream, item: TokenStream) -> Result<TokenStream, Error> {
    let definition = attr
        .into_iter()
        .next()
        .expect("expected api definition in braces");

    let definition = match definition {
        TokenTree::Group(ref group) if group.delimiter() == Delimiter::Brace => group.stream(),
        _ => c_bail!(definition => "expected api definition in braces"),
    };

    let def_span = definition.span();
    let definition = parse_object(definition)?;

    // Now parse the item, based on which we decide whether this is an API method which needs a
    // wrapper, or an API type which needs an ApiType implementation!
    let mut item: syn::Item = syn::parse2(item).unwrap();

    match item {
        syn::Item::Struct(mut itemstruct) => {
            let extra = struct_types::handle_struct(definition, &mut itemstruct)?;
            let mut output = itemstruct.into_token_stream();
            output.extend(extra);
            Ok(output)
        }
        syn::Item::Fn(func) => function::handle_function(def_span, definition, func),
        syn::Item::Enum(ref mut itemenum) => {
            let extra = enum_types::handle_enum(definition, itemenum)?;
            let mut output = item.into_token_stream();
            output.extend(extra);
            Ok(output)
        }
        _ => c_bail!(item => "api macro currently only applies to structs and functions"),
    }
}
