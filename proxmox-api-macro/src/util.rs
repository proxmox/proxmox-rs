use std::borrow::Borrow;
use std::fmt;

use proc_macro2::{Ident, TokenStream};
use syn::parse::{Parse, ParseStream};
use syn::Token;

/// A more relaxed version of Ident which allows hyphens.
#[derive(Clone, Debug)]
pub struct SimpleIdent(Ident, String);

impl SimpleIdent {
    //pub fn new(name: String, span: Span) -> Self {
    //    Self(Ident::new(&name, span), name)
    //}

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.1
    }

    //#[inline]
    //pub fn span(&self) -> Span {
    //    self.0.span()
    //}
}

impl Eq for SimpleIdent {}

impl PartialEq for SimpleIdent {
    fn eq(&self, other: &Self) -> bool {
        self.1 == other.1
    }
}

impl std::hash::Hash for SimpleIdent {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.1, state)
    }
}

impl From<Ident> for SimpleIdent {
    fn from(ident: Ident) -> Self {
        let s = ident.to_string();
        Self(ident, s)
    }
}

impl From<SimpleIdent> for Ident {
    fn from(this: SimpleIdent) -> Ident {
        this.0
    }
}

impl fmt::Display for SimpleIdent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl std::ops::Deref for SimpleIdent {
    type Target = Ident;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for SimpleIdent {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Borrow<str> for SimpleIdent {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl quote::ToTokens for SimpleIdent {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens)
    }
}

/// Note that the 'type' keyword is handled separately in `syn`. It's not an `Ident`:
impl Parse for SimpleIdent {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        Ok(Self::from(if lookahead.peek(Token![type]) {
            let ty: Token![type] = input.parse()?;
            Ident::new("type", ty.span)
        } else if lookahead.peek(syn::LitStr) {
            let s: syn::LitStr = input.parse()?;
            Ident::new(&s.value(), s.span())
        } else {
            input.parse()?
        }))
    }
}
