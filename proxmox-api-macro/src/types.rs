//! Helper types not found in proc_macro.
//!
//! Like a `Name` type which is like an `Ident` but allows hyphens, and is hashable so it can be
//! used as a key in a `HashMap`.

use proc_macro2::{Ident, Span, TokenStream};

use anyhow::Error;

/// A more relaxed version of Ident which allows hyphens.
#[derive(Clone, Debug)]
pub struct Name(String, Span);

impl Name {
    pub fn new(name: String, span: Span) -> Result<Self, Error> {
        let beg = name.as_bytes()[0];
        if !(beg.is_ascii_alphanumeric() || beg == b'_')
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            c_bail!(span, "`{}` is not a valid name", name);
        }
        Ok(Self(name, span))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn span(&self) -> Span {
        self.1
    }
}

impl From<Ident> for Name {
    fn from(ident: Ident) -> Name {
        Name(ident.to_string(), ident.span())
    }
}

impl PartialEq for Name {
    fn eq(&self, rhs: &Self) -> bool {
        self.0 == rhs.0
    }
}

impl Eq for Name {}

impl quote::ToTokens for Name {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        Ident::new(&self.0, self.1).to_tokens(tokens)
    }
}

impl std::borrow::Borrow<String> for Name {
    fn borrow(&self) -> &String {
        &self.0
    }
}

impl std::borrow::Borrow<str> for Name {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Name {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::hash::Hash for Name {
    fn hash<H>(&self, state: &mut H)
    where
        H: std::hash::Hasher,
    {
        std::hash::Hash::hash::<H>(&self.0, state)
    }
}
