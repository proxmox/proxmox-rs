use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{Meta, NestedMeta};

use crate::util::{self, default_false, parse_str_value_to_option, set_bool};

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

        util::extract_attributes(input, "updater", |attr, meta| this.parse(attr, meta));

        this
    }

    fn parse(&mut self, attr: &syn::Attribute, input: NestedMeta) -> Result<(), syn::Error> {
        match input {
            NestedMeta::Lit(lit) => bail!(lit => "unexpected literal"),
            NestedMeta::Meta(meta) => self.parse_meta(attr, meta),
        }
    }

    fn parse_meta(&mut self, attr: &syn::Attribute, meta: Meta) -> Result<(), syn::Error> {
        match meta {
            Meta::Path(ref path) if path.is_ident("skip") => {
                set_bool(&mut self.skip, path, true);
            }
            Meta::NameValue(ref nv) if nv.path.is_ident("type") => {
                parse_str_value_to_option(&mut self.ty, nv)
            }
            Meta::NameValue(m) => bail!(&m => "invalid updater attribute: {:?}", m.path),
            Meta::List(m) if m.path.is_ident("serde") => {
                let mut tokens = TokenStream::new();
                m.paren_token
                    .surround(&mut tokens, |tokens| m.nested.to_tokens(tokens));
                self.serde.push(syn::Attribute {
                    path: m.path,
                    tokens,
                    ..attr.clone()
                });
            }
            Meta::List(m) => bail!(&m => "invalid updater attribute: {:?}", m.path),
            Meta::Path(m) => bail!(&m => "invalid updater attribute: {:?}", m),
        }

        Ok(())
    }

    pub fn skip(&self) -> bool {
        default_false(self.skip.as_ref())
    }

    pub fn ty(&self) -> Option<&syn::TypePath> {
        self.ty.as_ref()
    }

    pub fn replace_serde_attributes(&self, attrs: &mut Vec<syn::Attribute>) {
        if !self.serde.is_empty() {
            attrs.retain(|attr| !attr.path.is_ident("serde"));
            attrs.extend(self.serde.iter().cloned())
        }
    }
}
