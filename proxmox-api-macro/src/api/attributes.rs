use proc_macro2::TokenStream;
use syn::meta::ParseNestedMeta;

use crate::util;

#[derive(Default)]
pub struct UpdaterFieldAttributes {
    /// Skip this field in the updater.
    skip: Option<syn::LitBool>,

    /// Change the type for the updater.
    ty: Option<syn::TypePath>,

    /// Replace any `#[serde]` attributes on the field with these (accumulates).
    serde: Vec<syn::Attribute>,
}

impl UpdaterFieldAttributes {
    pub fn from_attributes(input: &mut Vec<syn::Attribute>) -> Self {
        let mut this = Self::default();

        for attr in std::mem::take(input) {
            if attr.style != syn::AttrStyle::Outer || !attr.path().is_ident("updater") {
                input.push(attr);
                continue;
            }
            match attr.parse_nested_meta(|meta| this.parse(meta)) {
                Ok(()) => (),
                Err(err) => crate::add_error(err),
            }
        }

        this
    }

    fn parse(&mut self, meta: ParseNestedMeta<'_>) -> Result<(), syn::Error> {
        let path = &meta.path;

        if path.is_ident("skip") {
            if !meta.input.is_empty() {
                return Err(meta.error("'skip' attribute does not take any data"));
            }
            util::set_bool(&mut self.skip, path, true);
        } else if path.is_ident("type") {
            util::parse_str_value_to_option(&mut self.ty, path, meta.value()?);
        } else if path.is_ident("serde") {
            let content: TokenStream = meta.input.parse()?;
            self.serde.push(syn::parse_quote! { # [ #path #content ] });
        } else {
            return Err(meta.error(format!("invalid updater attribute: {path:?}")));
        }

        Ok(())
    }

    pub fn skip(&self) -> bool {
        util::default_false(self.skip.as_ref())
    }

    pub fn ty(&self) -> Option<&syn::TypePath> {
        self.ty.as_ref()
    }

    pub fn replace_serde_attributes(&self, attrs: &mut Vec<syn::Attribute>) {
        if !self.serde.is_empty() {
            attrs.retain(|attr| !attr.path().is_ident("serde"));
            attrs.extend(self.serde.iter().cloned())
        }
    }
}

#[derive(Default)]
pub struct EnumFieldAttributes {
    /// Change the "type-key" for this entry type..
    type_key: Option<syn::LitStr>,
}

impl EnumFieldAttributes {
    pub fn from_attributes(input: &mut Vec<syn::Attribute>) -> Self {
        let mut this = Self::default();

        for attr in std::mem::take(input) {
            if attr.style != syn::AttrStyle::Outer || !attr.path().is_ident("api") {
                input.push(attr);
                continue;
            }
            match attr.parse_nested_meta(|meta| this.parse(meta)) {
                Ok(()) => (),
                Err(err) => crate::add_error(err),
            }
        }

        this
    }

    fn parse(&mut self, meta: ParseNestedMeta<'_>) -> Result<(), syn::Error> {
        let path = &meta.path;

        if path.is_ident("type_key") {
            util::duplicate(&self.type_key, path);
            self.type_key = Some(meta.value()?.parse()?);
        } else {
            return Err(meta.error(format!("invalid api attribute: {path:?}")));
        }

        Ok(())
    }

    pub fn type_key(&self) -> Option<&syn::LitStr> {
        self.type_key.as_ref()
    }
}
