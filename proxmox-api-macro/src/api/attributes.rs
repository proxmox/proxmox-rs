use syn::{Meta, NestedMeta};

use crate::util::{self, default_false, set_bool};

#[derive(Default)]
pub struct UpdaterFieldAttributes {
    /// Skip this field in the updater.
    skip: Option<syn::LitBool>,
    // /// Change the type for the updater.
    // ty: Option<syn::Type>,
}

impl UpdaterFieldAttributes {
    pub fn from_attributes(input: &mut Vec<syn::Attribute>) -> Self {
        let mut this = Self::default();

        util::extract_attributes(input, "updater", |meta| this.parse(meta));

        this
    }

    fn parse(&mut self, input: NestedMeta) -> Result<(), syn::Error> {
        match input {
            NestedMeta::Lit(lit) => bail!(lit => "unexpected literal"),
            NestedMeta::Meta(meta) => self.parse_meta(meta),
        }
    }

    fn parse_meta(&mut self, meta: Meta) -> Result<(), syn::Error> {
        match meta {
            Meta::Path(ref path) if path.is_ident("skip") => {
                set_bool(&mut self.skip, path, true);
            }
            // Meta::NameValue(ref nv) if nv.path.is_ident("type") => {
            //     parse_str_value_to_option(&mut self.ty, nv)
            // }
            Meta::NameValue(m) => bail!(&m => "invalid updater attribute: {:?}", m.path),
            Meta::List(m) => bail!(&m => "invalid updater attribute: {:?}", m.path),
            Meta::Path(m) => bail!(&m => "invalid updater attribute: {:?}", m),
        }

        Ok(())
    }

    pub fn skip(&self) -> bool {
        default_false(self.skip.as_ref())
    }

    //pub fn ty(&self) -> Option<&syn::Type> {
    //    self.ty.as_ref()
    //}
}
