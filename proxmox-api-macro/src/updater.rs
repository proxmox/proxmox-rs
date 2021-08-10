use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

pub(crate) fn updatable(item: TokenStream) -> Result<TokenStream, syn::Error> {
    let item: syn::Item = syn::parse2(item)?;
    let full_span = item.span();
    match item {
        syn::Item::Struct(syn::ItemStruct {
            fields: syn::Fields::Named(named),
            attrs,
            ident,
            generics,
            ..
        }) => derive_named_struct_updatable(attrs, full_span, ident, generics, named),
        syn::Item::Struct(syn::ItemStruct {
            attrs,
            ident,
            generics,
            ..
        }) => derive_default_updatable(attrs, full_span, ident, generics),
        syn::Item::Enum(syn::ItemEnum {
            attrs,
            ident,
            generics,
            ..
        }) => derive_default_updatable(attrs, full_span, ident, generics),
        _ => bail!(item => "`Updatable` can only be derived for structs"),
    }
}

fn no_generics(generics: syn::Generics) {
    if let Some(lt) = generics.lt_token {
        error!(lt => "deriving `Updatable` for a generic enum is not supported");
    } else if let Some(wh) = generics.where_clause {
        error!(wh => "deriving `Updatable` on enums with generic bounds is not supported");
    }
}

fn derive_default_updatable(
    attrs: Vec<syn::Attribute>,
    full_span: Span,
    ident: Ident,
    generics: syn::Generics,
) -> Result<TokenStream, syn::Error> {
    no_generics(generics);

    let args = UpdatableArgs::from_attributes(attrs);
    if let Some(updater) = args.updater {
        error!(updater => "`updater` updater attribute not supported for this type");
    }

    Ok(default_updatable(full_span, ident))
}

fn default_updatable(full_span: Span, ident: Ident) -> TokenStream {
    quote_spanned! { full_span =>
        #[automatically_derived]
        impl ::proxmox::api::schema::Updatable for #ident {
            type Updater = Option<#ident>;
            const UPDATER_IS_OPTION: bool = true;
        }
    }
}

fn derive_named_struct_updatable(
    attrs: Vec<syn::Attribute>,
    full_span: Span,
    ident: Ident,
    generics: syn::Generics,
    _fields: syn::FieldsNamed,
) -> Result<TokenStream, syn::Error> {
    no_generics(generics);

    let args = UpdatableArgs::from_attributes(attrs);
    let updater = match args.updater {
        Some(updater) => updater,
        None => return Ok(default_updatable(full_span, ident)),
    };

    Ok(quote! {
        #[automatically_derived]
        impl ::proxmox::api::schema::Updatable for #ident {
            type Updater = #updater;
            const UPDATER_IS_OPTION: bool = false;
        }
    })
}

#[derive(Default)]
struct UpdatableArgs {
    updater: Option<syn::Type>,
}

impl UpdatableArgs {
    fn from_attributes(attributes: Vec<syn::Attribute>) -> Self {
        let mut this = Self::default();

        for_attributes(attributes, "updatable", |meta| this.parse_nested(meta));

        this
    }

    fn parse_nested(&mut self, meta: syn::NestedMeta) -> Result<(), syn::Error> {
        match meta {
            syn::NestedMeta::Meta(syn::Meta::NameValue(nv)) => self.parse_name_value(nv),
            other => bail!(other => "invalid updater argument"),
        }
    }

    fn parse_name_value(&mut self, nv: syn::MetaNameValue) -> Result<(), syn::Error> {
        if nv.path.is_ident("updater") {
            let updater: syn::Type = match nv.lit {
                // we could use `s.parse()` but it doesn't make sense to put the original struct
                // name as spanning info here, so instead, we use the call site:
                syn::Lit::Str(s) => syn::parse_str(&s.value())?,
                other => bail!(other => "updater argument must be a string literal"),
            };

            if self.updater.is_some() {
                error!(updater.span(), "multiple 'updater' attributes not allowed");
            }

            self.updater = Some(updater);
            Ok(())
        } else {
            bail!(nv.path => "unrecognized updater argument");
        }
    }
}

/// Non-fatally go through all `updater` attributes.
fn for_attributes<F>(attributes: Vec<syn::Attribute>, attr_name: &str, mut func: F)
where
    F: FnMut(syn::NestedMeta) -> Result<(), syn::Error>,
{
    for meta in meta_iter(attributes) {
        let list = match meta {
            syn::Meta::List(list) if list.path.is_ident(attr_name) => list,
            _ => continue,
        };

        for entry in list.nested {
            match func(entry) {
                Ok(()) => (),
                Err(err) => crate::add_error(err),
            }
        }
    }
}

fn meta_iter(
    attributes: impl IntoIterator<Item = syn::Attribute>,
) -> impl Iterator<Item = syn::Meta> {
    attributes.into_iter().filter_map(|attr| {
        if attr.style != syn::AttrStyle::Outer {
            return None;
        }

        attr.parse_meta().ok()
    })
}
