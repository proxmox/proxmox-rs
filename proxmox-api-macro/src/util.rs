use std::borrow::Borrow;
use std::collections::HashMap;
use std::convert::TryFrom;

use proc_macro2::{Ident, Span};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::Token;

use anyhow::Error;

use crate::api::{self, Schema, SchemaItem};

/// A more relaxed version of Ident which allows hyphens.
///
/// Note that this acts both as an Ident and as a String so that we can easily access an &str
/// (which Ident does not provide, instead, Ident always requires you to produce a newly owned
/// `String`).
/// Because of this the user also needs to be aware of the differences between idents and strings,
/// and therefore we do not implement `Into<Ident>` anymore, but the user needs to explicitly ask
/// for it via the `.into_ident()` method.
#[derive(Clone, Debug)]
pub struct FieldName {
    ident: Ident,
    ident_str: String, // cached string version to avoid all the .to_string() calls
    string: String,    // hyphenated version
}

impl FieldName {
    pub fn new(name: String, span: Span) -> Self {
        let mut ident_str = name.replace(['-', '.', '+'].as_ref(), "_");

        if ident_str.chars().next().unwrap().is_numeric() {
            ident_str.insert(0, '_');
        }

        Self {
            ident: Ident::new(&ident_str, span),
            ident_str,
            string: name,
        }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.string
    }

    #[inline]
    pub fn as_ident_str(&self) -> &str {
        &self.ident_str
    }

    #[inline]
    pub fn as_ident(&self) -> &Ident {
        &self.ident
    }

    #[inline]
    pub fn into_ident(self) -> Ident {
        self.ident
    }

    #[inline]
    pub fn span(&self) -> Span {
        self.ident.span()
    }

    pub fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.string.cmp(&other.string)
    }

    pub fn into_lit_str(self) -> syn::LitStr {
        syn::LitStr::new(&self.string, self.ident.span())
    }

    pub fn into_str(self) -> String {
        self.string
    }
}

impl Eq for FieldName {}

impl PartialEq for FieldName {
    fn eq(&self, other: &Self) -> bool {
        self.string == other.string
    }
}

impl std::hash::Hash for FieldName {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.string, state)
    }
}

impl From<Ident> for FieldName {
    fn from(ident: Ident) -> Self {
        let string = ident.to_string();
        Self {
            ident,
            ident_str: string.clone(),
            string,
        }
    }
}

impl From<&syn::LitStr> for FieldName {
    fn from(s: &syn::LitStr) -> Self {
        Self::new(s.value(), s.span())
    }
}

impl Borrow<str> for FieldName {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

/// Note that the 'type' keyword is handled separately in `syn`. It's not an `Ident`:
impl Parse for FieldName {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        Ok(if lookahead.peek(Token![type]) {
            let ty: Token![type] = input.parse()?;
            Self::new("type".to_string(), ty.span)
        } else if lookahead.peek(syn::LitStr) {
            let s: syn::LitStr = input.parse()?;
            Self::new(s.value(), s.span())
        } else {
            let ident: Ident = input.parse()?;
            Self::from(ident)
        })
    }
}

/// Most of our schema definition consists of a json-like notation.
/// For parsing we mostly just need to destinguish between objects and non-objects.
/// For specific expression types we match on the contained expression later on.
// FIXME: Expr(Box<syn::Expr>)
#[allow(clippy::large_enum_variant)]
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
impl TryFrom<JSONValue> for FieldName {
    type Error = syn::Error;
    fn try_from(value: JSONValue) -> Result<Self, syn::Error> {
        Ok(FieldName::from(Ident::try_from(value)?))
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
    pub elements: HashMap<FieldName, JSONValue>,
}

impl JSONObject {
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    fn parse_elements(input: ParseStream) -> syn::Result<HashMap<FieldName, JSONValue>> {
        let map_elems: Punctuated<JSONMapEntry, Token![,]> =
            input.parse_terminated(JSONMapEntry::parse)?;
        let mut elems = HashMap::with_capacity(map_elems.len());
        for c in map_elems {
            if elems.insert(c.key.clone(), c.value).is_some() {
                bail!(c.key.span(), "duplicate '{}' in schema", c.key.as_str());
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
    type Target = HashMap<FieldName, JSONValue>;

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
    type Item = <HashMap<FieldName, JSONValue> as IntoIterator>::Item;
    type IntoIter = <HashMap<FieldName, JSONValue> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.elements.into_iter()
    }
}

/// An element in a json style map.
struct JSONMapEntry {
    pub key: FieldName,
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

pub fn get_doc_comments(attributes: &[syn::Attribute]) -> Result<(String, Span), syn::Error> {
    let mut doc_comment = String::new();
    let doc_span = Span::call_site(); // FIXME: set to first doc comment

    for attr in attributes {
        // skip #![...]
        if let syn::AttrStyle::Inner(_) = &attr.style {
            continue;
        }

        if attr.path.is_ident("doc") {
            let doc: BareAssignment<syn::LitStr> = syn::parse2(attr.tokens.clone())?;
            if !doc_comment.is_empty() {
                doc_comment.push('\n');
            }
            doc_comment.push_str(doc.content.value().trim());
        }
    }

    Ok((doc_comment, doc_span))
}

pub fn derive_descriptions(
    input_schema: &mut Schema,
    returns_schema: &mut Option<Schema>,
    doc_comment: &str,
    doc_span: Span,
) -> Result<(), Error> {
    // If we have a doc comment, allow automatically inferring the description for the input and
    // output objects:
    if doc_comment.is_empty() {
        return Ok(());
    }

    let mut parts = doc_comment.split("\nReturns:");

    if let Some(first) = parts.next() {
        if input_schema.description.is_none() {
            input_schema.description = Some(syn::LitStr::new(first.trim(), doc_span));
        }
    }

    if let Some(second) = parts.next() {
        if let Some(ref mut returns_schema) = returns_schema {
            if returns_schema.description.is_none() {
                returns_schema.description = Some(syn::LitStr::new(second.trim(), doc_span));
            }
        }

        if parts.next().is_some() {
            bail!(
                doc_span,
                "multiple 'Returns:' sections found in doc comment!"
            );
        }
    }

    Ok(())
}

pub fn infer_type(schema: &mut Schema, ty: &syn::Type) -> Result<bool, syn::Error> {
    if let SchemaItem::Inferred(_) = schema.item {
        //
    } else {
        return Ok(is_option_type(ty).is_some());
    }

    let (ty, is_option) = match is_option_type(ty) {
        Some(ty) => (ty, true),
        None => (ty, false),
    };

    // infer the type from a rust type:
    match ty {
        syn::Type::Path(path) if path.qself.is_none() => {
            if path.path.is_ident("String") {
                schema.item = SchemaItem::String;
            } else if path.path.is_ident("bool") {
                schema.item = SchemaItem::Boolean;
            } else if let Some(ty) = api::INTTYPES.iter().find(|i| path.path.is_ident(i.name)) {
                schema.item = SchemaItem::Integer;
                if let Some(min) = ty.minimum {
                    schema.add_default_property("minimum", syn::Expr::Verbatim(min.parse()?));
                }
                if let Some(max) = ty.maximum {
                    schema.add_default_property("maximum", syn::Expr::Verbatim(max.parse()?));
                }
            } else if api::NUMBERNAMES.iter().any(|n| path.path.is_ident(n)) {
                schema.item = SchemaItem::Number;
            } else {
                bail!(ty => "cannot infer parameter type from this rust type");
            }
        }
        _ => (),
    }

    Ok(is_option)
}

/// Note that we cannot handle renamed imports at all here...
pub fn is_option_type(ty: &syn::Type) -> Option<&syn::Type> {
    if let syn::Type::Path(p) = ty {
        if p.qself.is_some() {
            return None;
        }
        let segs = &p.path.segments;
        let is_option = match segs.len() {
            1 => segs.last().unwrap().ident == "Option",
            2 => segs.first().unwrap().ident == "std" && segs.last().unwrap().ident == "Option",
            _ => false,
        };
        if !is_option {
            return None;
        }

        if let syn::PathArguments::AngleBracketed(generic) = &segs.last().unwrap().arguments {
            if generic.args.len() == 1 {
                if let syn::GenericArgument::Type(ty) = generic.args.first().unwrap() {
                    return Some(ty);
                }
            }
        }
    }
    None
}

/// Helper to do basically what `parse_macro_input!` does, except the macro expects a
/// `TokenStream_1`, and we always have a `TokenStream` from `proc_macro2`.
pub struct AttrArgs {
    _paren_token: syn::token::Paren,
    pub args: Punctuated<syn::NestedMeta, Token![,]>,
}

impl Parse for AttrArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            _paren_token: syn::parenthesized!(content in input),
            args: Punctuated::parse_terminated(&content)?,
        })
    }
}

/// Join an iterator over `Display` values.
pub fn join<T>(separator: &str, iter: impl Iterator<Item = T>) -> String
where
    T: std::fmt::Display,
{
    let mut text = String::new();
    let mut sep = "";
    for i in iter {
        text = format!("{}{}{}", text, sep, i);
        sep = separator;
    }
    text
}

/// Join an iterator over `Debug` values.
pub fn join_debug<T>(separator: &str, iter: impl Iterator<Item = T>) -> String
where
    T: std::fmt::Debug,
{
    let mut text = String::new();
    let mut sep = "";
    for i in iter {
        text = format!("{}{}{:?}", text, sep, i);
        sep = separator;
    }
    text
}
