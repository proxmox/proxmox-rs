use proc_macro2::{Ident, Span, TokenStream};
use quote::quote_spanned;
use syn::spanned::Spanned;

pub(crate) fn updater_type(item: TokenStream) -> Result<TokenStream, syn::Error> {
    let item: syn::Item = syn::parse2(item)?;
    let full_span = item.span();
    Ok(match item {
        syn::Item::Struct(syn::ItemStruct {
            ident, generics, ..
        }) => derive_updater_type(full_span, ident, generics),
        syn::Item::Enum(syn::ItemEnum {
            ident, generics, ..
        }) => derive_updater_type(full_span, ident, generics),
        _ => bail!(item => "`UpdaterType` cannot be derived for this type"),
    })
}

fn no_generics(generics: syn::Generics) {
    if let Some(lt) = generics.lt_token {
        error!(lt => "deriving `UpdaterType` for a generic enum is not supported");
    } else if let Some(wh) = generics.where_clause {
        error!(wh => "deriving `UpdaterType` on enums with generic bounds is not supported");
    }
}

fn derive_updater_type(full_span: Span, ident: Ident, generics: syn::Generics) -> TokenStream {
    no_generics(generics);

    quote_spanned! { full_span =>
        impl ::proxmox_schema::UpdaterType for #ident {
            type Updater = Option<Self>;
        }
    }
}
