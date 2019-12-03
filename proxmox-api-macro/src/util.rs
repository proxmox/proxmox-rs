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
///
/// Note that this acts both as an Ident and as a String so that we can easily access an &str
/// (which Ident does not provide, instead, Ident always requires you to produce a newly owned
/// `String`).
/// Because of this the user also needs to be aware of the differences between idents and strings,
/// and therefore we do not implement `Into<Ident>` anymore, but the user needs to explicitly ask
/// for it via the `.into_ident()` method.
#[derive(Clone, Debug)]
pub struct SimpleIdent(Ident, String);

impl SimpleIdent {
    pub fn new(name: String, span: Span) -> Self {
        Self(Ident::new(&name.replace("-", "_"), span), name)
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.1
    }

    #[inline]
    pub unsafe fn into_ident_unchecked(self) -> Ident {
        self.0
    }

    #[inline]
    pub fn into_ident(self) -> Result<Ident, syn::Error> {
        if self.1.as_bytes().contains(&b'-') {
            bail!(self.0 => "invalid identifier: '{}'", self.1);
        }
        Ok(unsafe { self.into_ident_unchecked() })
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

impl fmt::Display for SimpleIdent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
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

    pub fn new_string(value: &str, span: Span) -> JSONValue {
        JSONValue::Expr(syn::Expr::Lit(syn::ExprLit {
            attrs: Vec::new(),
            lit: syn::Lit::Str(syn::LitStr::new(value, span)),
        }))
    }

    pub fn new_ident(ident: Ident) -> JSONValue {
        JSONValue::Expr(syn::Expr::Path(syn::ExprPath {
            attrs: Vec::new(),
            qself: None,
            path: syn::Path {
                leading_colon: None,
                segments: {
                    let mut p = Punctuated::new();
                    p.push(syn::PathSegment {
                        ident,
                        arguments: Default::default(),
                    });
                    p
                },
            },
        }))
    }

    pub fn span(&self) -> Span {
        match self {
            JSONValue::Object(obj) => obj.brace_token.span,
            JSONValue::Expr(expr) => expr.span(),
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

/// Expect a json value to be a path. This means it's supposed to be an expression which evaluates
/// to a path.
impl TryFrom<JSONValue> for syn::ExprPath {
    type Error = syn::Error;
    fn try_from(value: JSONValue) -> Result<Self, syn::Error> {
        match syn::Expr::try_from(value)? {
            syn::Expr::Path(path) => Ok(path),
            other => bail!(other => "expected a type path"),
        }
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
    fn parse_elements(input: ParseStream) -> syn::Result<HashMap<SimpleIdent, JSONValue>> {
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

    pub fn parse_inner(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            brace_token: syn::token::Brace {
                span: Span::call_site(),
            },
            elements: Self::parse_elements(input)?,
        })
    }
}

impl Parse for JSONObject {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            brace_token: syn::braced!(content in input),
            elements: Self::parse_elements(&content)?,
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

/// We get macro attributes like `#[doc = "TEXT"]` with the `=` included.
pub struct BareAssignment<T: Parse> {
    pub token: Token![=],
    pub content: T,
}

impl<T: Parse> Parse for BareAssignment<T> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            token: input.parse()?,
            content: input.parse()?,
        })
    }
}
