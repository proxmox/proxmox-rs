use std::convert::TryFrom;

use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

use crate::serde;
use crate::util;

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

            fn update_from<T: AsRef<str>>(
                &mut self,
                from: Option<#ident>,
                _delete: &[T],
            ) -> Result<(), ::anyhow::Error> {
                if let Some(val) = from {
                    *self = val;
                }

                Ok(())
            }

            fn try_build_from(from: Option<#ident>) -> Result<Self, ::anyhow::Error> {
                from.ok_or_else(|| ::anyhow::format_err!("cannot build from None value"))
            }
        }
    }
}

fn derive_named_struct_updatable(
    attrs: Vec<syn::Attribute>,
    full_span: Span,
    ident: Ident,
    generics: syn::Generics,
    fields: syn::FieldsNamed,
) -> Result<TokenStream, syn::Error> {
    no_generics(generics);

    let serde_container_attrs = serde::ContainerAttrib::try_from(&attrs[..])?;
    let args = UpdatableArgs::from_attributes(attrs);
    let updater = match args.updater {
        Some(updater) => updater,
        None => return Ok(default_updatable(full_span, ident)),
    };

    let mut delete = TokenStream::new();
    let mut apply = TokenStream::new();
    let mut build = TokenStream::new();

    for field in fields.named {
        let serde_attrs = serde::SerdeAttrib::try_from(&field.attrs[..])?;
        let attrs = UpdaterFieldArgs::from_attributes(field.attrs);

        let field_ident = field
            .ident
            .as_ref()
            .expect("unnamed field in named struct?");

        let field_name_string = if let Some(renamed) = serde_attrs.rename {
            renamed.into_str()
        } else if let Some(rename_all) = serde_container_attrs.rename_all {
            let name = rename_all.apply_to_field(&field_ident.to_string());
            name
        } else {
            field_ident.to_string()
        };

        let build_err = format!(
            "failed to build value for field '{}': {{}}",
            field_name_string
        );
        if util::is_option_type(&field.ty).is_some() {
            delete.extend(quote! {
                #field_name_string => { self.#field_ident = None; }
            });
            build.extend(quote! {
                #field_ident: ::proxmox::api::schema::Updatable::try_build_from(
                    from.#field_ident
                )
                .map_err(|err| ::anyhow::format_err!(#build_err, err))?,
            });
        } else {
            build.extend(quote! {
                #field_ident: ::proxmox::api::schema::Updatable::try_build_from(
                    from.#field_ident
                )
                .map_err(|err| ::anyhow::format_err!(#build_err, err))?,
            });
        }

        if attrs.fixed {
            let error = format!(
                "field '{}' must not be set when updating existing data",
                field_ident
            );
            apply.extend(quote! {
                if from.#field_ident.is_some() {
                    ::anyhow::bail!(#error);
                }
            });
        } else {
            apply.extend(quote! {
                ::proxmox::api::schema::Updatable::update_from(
                    &mut self.#field_ident,
                    from.#field_ident,
                    delete,
                )?;
            });
        }
    }

    if !delete.is_empty() {
        delete = quote! {
            for delete in delete {
                match delete.as_ref() {
                    #delete
                    _ => continue,
                }
            }
        };
    }

    Ok(quote! {
        #[automatically_derived]
        impl ::proxmox::api::schema::Updatable for #ident {
            type Updater = #updater;
            const UPDATER_IS_OPTION: bool = false;

            fn update_from<T: AsRef<str>>(
                &mut self,
                from: Self::Updater,
                delete: &[T],
            ) -> Result<(), ::anyhow::Error> {
                #delete
                #apply
                Ok(())
            }

            fn try_build_from(from: Self::Updater) -> Result<Self, ::anyhow::Error> {
                Ok(Self {
                    #build
                })
            }
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

#[derive(Default)]
struct UpdaterFieldArgs {
    /// A fixed field must not be set in the `Updater` when the data is updated via `update_from`,
    /// but is still required for the `build()` method.
    fixed: bool,
}

impl UpdaterFieldArgs {
    fn from_attributes(attributes: Vec<syn::Attribute>) -> Self {
        let mut this = Self::default();
        for_attributes(attributes, "updater", |meta| this.parse_nested(meta));
        this
    }

    fn parse_nested(&mut self, meta: syn::NestedMeta) -> Result<(), syn::Error> {
        match meta {
            syn::NestedMeta::Meta(syn::Meta::Path(path)) if path.is_ident("fixed") => {
                self.fixed = true;
            }
            other => bail!(other => "invalid updater argument"),
        }
        Ok(())
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
