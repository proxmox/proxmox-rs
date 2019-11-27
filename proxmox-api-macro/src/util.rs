use std::borrow::Borrow;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;

use proc_macro2::{Ident, Span, TokenStream};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
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

/// Most of our schema definition consists of a json-like notation.
/// For parsing we mostly just need to destinguish between objects and non-objects.
/// For specific expression types we match on the contained expression later on.
pub enum JSONValue {
    Object(JSONObject),
    Expr(syn::Expr),
}

impl JSONValue {
    /// When we expect an object, it's nicer to know why/what kind, so instead of
    /// `TryInto<JSONObject>` we provide this method:
    pub fn into_object(self, expected: &str) -> Result<JSONObject, syn::Error> {
        match self {
            JSONValue::Object(s) => Ok(s),
            JSONValue::Expr(e) => bail!(e => "expected {}", expected),
        }
    }
}

/// Expect a json value to be an expression, not an object:
impl TryFrom<JSONValue> for syn::Expr {
    type Error = syn::Error;
    fn try_from(value: JSONValue) -> Result<Self, syn::Error> {
        match value {
            JSONValue::Object(s) => bail!(s.brace_token.span, "unexpected object"),
            JSONValue::Expr(e) => Ok(e),
        }
    }
}

/// Expect a json value to be a literal string:
impl TryFrom<JSONValue> for syn::LitStr {
    type Error = syn::Error;
    fn try_from(value: JSONValue) -> Result<Self, syn::Error> {
        let expr = syn::Expr::try_from(value)?;
        if let syn::Expr::Lit(lit) = expr {
            if let syn::Lit::Str(lit) = lit.lit {
                return Ok(lit);
            }
            bail!(lit => "expected string literal");
        }
        bail!(expr => "expected string literal");
    }
}

/// Expect a json value to be a literal boolean:
impl TryFrom<JSONValue> for syn::LitBool {
    type Error = syn::Error;
    fn try_from(value: JSONValue) -> Result<Self, syn::Error> {
        let expr = syn::Expr::try_from(value)?;
        if let syn::Expr::Lit(lit) = expr {
            if let syn::Lit::Bool(lit) = lit.lit {
                return Ok(lit);
            }
            bail!(lit => "expected literal boolean");
        }
        bail!(expr => "expected literal boolean");
    }
}

/// Expect a json value to be a literal boolean:
impl TryFrom<JSONValue> for bool {
    type Error = syn::Error;
    fn try_from(value: JSONValue) -> Result<Self, syn::Error> {
        Ok(syn::LitBool::try_from(value)?.value)
    }
}

/// Expect a json value to be an identifier:
impl TryFrom<JSONValue> for Ident {
    type Error = syn::Error;
    fn try_from(value: JSONValue) -> Result<Self, syn::Error> {
        let expr = syn::Expr::try_from(value)?;
        let span = expr.span();
        if let syn::Expr::Path(path) = expr {
            let mut iter = path.path.segments.into_pairs();
            let segment = iter
                .next()
                .ok_or_else(|| format_err!(span, "expected an identify, got an empty path"))?
                .into_value();
            if iter.next().is_some() {
                bail!(span, "expected an identifier, not a path");
            }
            if !segment.arguments.is_empty() {
                bail!(segment.arguments => "unexpected path arguments, expected an identifier");
            }
            return Ok(segment.ident);
        }
        bail!(expr => "expected an identifier");
    }
}

/// Expect a json value to be our "simple" identifier, which can be either an Ident or a String, or
/// the 'type' keyword:
impl TryFrom<JSONValue> for SimpleIdent {
    type Error = syn::Error;
    fn try_from(value: JSONValue) -> Result<Self, syn::Error> {
        Ok(SimpleIdent::from(Ident::try_from(value)?))
    }
}

/// Parsing a json value should be simple enough: braces means we have an object, otherwise it must
/// be an "expression".
impl Parse for JSONValue {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        Ok(if lookahead.peek(syn::token::Brace) {
            JSONValue::Object(input.parse()?)
        } else {
            JSONValue::Expr(input.parse()?)
        })
    }
}

/// The "core" of our schema is a json object.
pub struct JSONObject {
    pub brace_token: syn::token::Brace,
    pub elements: HashMap<SimpleIdent, JSONValue>,
}

impl JSONObject {
    fn parse_inner(input: ParseStream) -> syn::Result<HashMap<SimpleIdent, JSONValue>> {
        let map_elems: Punctuated<JSONMapEntry, Token![,]> =
            input.parse_terminated(JSONMapEntry::parse)?;
        let mut elems = HashMap::with_capacity(map_elems.len());
        for c in map_elems {
            if elems.insert(c.key.clone().into(), c.value).is_some() {
                bail!(&c.key => "duplicate '{}' in schema", c.key);
            }
        }
        Ok(elems)
    }
}

impl Parse for JSONObject {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            brace_token: syn::braced!(content in input),
            elements: Self::parse_inner(&content)?,
        })
    }
}

impl std::ops::Deref for JSONObject {
    type Target = HashMap<SimpleIdent, JSONValue>;

    fn deref(&self) -> &Self::Target {
        &self.elements
    }
}

impl std::ops::DerefMut for JSONObject {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.elements
    }
}

impl JSONObject {
    pub fn span(&self) -> Span {
        self.brace_token.span
    }

    pub fn remove_required_element(&mut self, name: &str) -> Result<JSONValue, syn::Error> {
        self.remove(name)
            .ok_or_else(|| format_err!(self.span(), "missing required element: {}", name))
    }
}

impl IntoIterator for JSONObject {
    type Item = <HashMap<SimpleIdent, JSONValue> as IntoIterator>::Item;
    type IntoIter = <HashMap<SimpleIdent, JSONValue> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.elements.into_iter()
    }
}

/// An element in a json style map.
struct JSONMapEntry {
    pub key: SimpleIdent,
    pub colon_token: Token![:],
    pub value: JSONValue,
}

impl Parse for JSONMapEntry {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            key: input.parse()?,
            colon_token: input.parse()?,
            value: input.parse()?,
        })
    }
}
