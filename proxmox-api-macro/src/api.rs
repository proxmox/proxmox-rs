extern crate proc_macro;
extern crate proc_macro2;

use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::mem;

use failure::Error;

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::Ident;
use syn::{parenthesized, Token};

use crate::util::SimpleIdent;

/// Most of our schema definition consists of a json-like notation.
/// For parsing we mostly just need to destinguish between objects and non-objects.
/// For specific expression types we match on the contained expression later on.
enum JSONValue {
    Object(JSONObject),
    Expr(syn::Expr),
}

impl JSONValue {
    /// When we expect an object, it's nicer to know why/what kind, so instead of
    /// `TryInto<JSONObject>` we provide this method:
    fn into_object(self, expected: &str) -> Result<JSONObject, syn::Error> {
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
struct JSONObject {
    pub brace_token: syn::token::Brace,
    pub elements: HashMap<SimpleIdent, JSONValue>,
}

impl Parse for JSONObject {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            brace_token: syn::braced!(content in input),
            elements: {
                let map_elems: Punctuated<JSONMapEntry, Token![,]> =
                    content.parse_terminated(JSONMapEntry::parse)?;
                let mut elems = HashMap::with_capacity(map_elems.len());
                for c in map_elems {
                    if elems.insert(c.key.clone().into(), c.value).is_some() {
                        bail!(&c.key => "duplicate '{}' in schema", c.key);
                    }
                }
                elems
            },
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
    fn span(&self) -> Span {
        self.brace_token.span
    }

    fn remove_required_element(&mut self, name: &str) -> Result<JSONValue, syn::Error> {
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

/// The main `Schema` type.
///
/// We have 2 fixed keys: `type` and `description`. The remaining keys depend on the `type`.
/// Generally, we create the following mapping:
///
/// ```text
/// {
///     type: Object,
///     description: "text",
///     foo: bar, // "unknown", will be added as a builder-pattern method
///     properties: { ... }
/// }
/// ```
///
/// to:
///
/// ```text
/// {
///     ObjectSchema::new("text", &[ ... ]).foo(bar)
/// }
/// ```
struct Schema {
    span: Span,

    /// Common in all schema entry types:
    description: Option<syn::LitStr>,

    /// The specific schema type (Object, String, ...)
    item: SchemaItem,

    /// The remaining key-value pairs the `SchemaItem` parser did not extract will be appended as
    /// builder-pattern method calls to this schema.
    properties: Vec<(Ident, syn::Expr)>,
}

/// We parse this in 2 steps: first we parse a `JSONValue`, then we "parse" that further.
impl Parse for Schema {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let obj: JSONObject = input.parse()?;
        Self::try_from(obj)
    }
}

/// Shortcut:
impl TryFrom<JSONValue> for Schema {
    type Error = syn::Error;

    fn try_from(value: JSONValue) -> Result<Self, syn::Error> {
        Self::try_from(value.into_object("a schema definition")?)
    }
}

/// To go from a `JSONObject` to a `Schema` we first extract the description, as it is a common
/// element in all schema entries, then we parse the specific `SchemaItem`, and collect all the
/// remaining "unused" keys as "constraints"/"properties" which will be appended as builder-pattern
/// method calls when translating the object to a schema definition.
impl TryFrom<JSONObject> for Schema {
    type Error = syn::Error;

    fn try_from(mut obj: JSONObject) -> Result<Self, syn::Error> {
        let description = obj
            .remove("description")
            .map(|v| v.try_into())
            .transpose()?;

        Ok(Self {
            span: obj.brace_token.span,
            description,
            item: SchemaItem::try_extract_from(&mut obj)?,
            properties: obj.into_iter().try_fold(
                Vec::new(),
                |mut properties, (key, value)| -> Result<_, syn::Error> {
                    properties.push((Ident::from(key), value.try_into()?));
                    Ok(properties)
                },
            )?,
        })
    }
}

impl Schema {
    fn to_schema(&self, ts: &mut TokenStream) -> Result<(), Error> {
        // First defer to the SchemaItem's `.to_schema()` method:
        let description = self
            .description
            .as_ref()
            .ok_or_else(|| format_err!(self.span, "missing description"))?;

        self.item.to_schema(ts, description)?;

        // Then append all the remaining builder-pattern properties:
        for prop in self.properties.iter() {
            let key = &prop.0;
            let value = &prop.1;
            ts.extend(quote! { .#key(#value) });
        }

        Ok(())
    }
}

enum SchemaItem {
    Null,
    Boolean,
    Integer,
    String,
    Object(SchemaObject),
    Array(SchemaArray),
}

impl SchemaItem {
    /// If there's a `type` specified, parse it as that type. Otherwise check for keys which
    /// uniqueply identify the type, such as "properties" for type `Object`.
    fn try_extract_from(obj: &mut JSONObject) -> Result<Self, syn::Error> {
        let ty = obj.remove("type").map(SimpleIdent::try_from).transpose()?;
        let ty = match &ty {
            Some(ty) => ty.as_str(),
            None => {
                if obj.contains_key("properties") {
                    "Object"
                } else if obj.contains_key("items") {
                    "Array"
                } else {
                    bail!(obj.span(), "failed to guess 'type' in schema definition");
                }
            }
        };
        match ty {
            "Null" => Ok(SchemaItem::Null),
            "Boolean" => Ok(SchemaItem::Boolean),
            "Integer" => Ok(SchemaItem::Integer),
            "String" => Ok(SchemaItem::String),
            "Object" => Ok(SchemaItem::Object(SchemaObject::try_extract_from(obj)?)),
            "Array" => Ok(SchemaItem::Array(SchemaArray::try_extract_from(obj)?)),
            ty => bail!(obj.span(), "unknown type name '{}'", ty),
        }
    }

    fn to_schema(&self, ts: &mut TokenStream, description: &syn::LitStr) -> Result<(), Error> {
        ts.extend(quote! { ::proxmox::api::schema });
        match self {
            SchemaItem::Null => ts.extend(quote! { ::NullSchema::new(#description) }),
            SchemaItem::Boolean => ts.extend(quote! { ::BooleanSchema::new(#description) }),
            SchemaItem::Integer => ts.extend(quote! { ::IntegerSchema::new(#description) }),
            SchemaItem::String => ts.extend(quote! { ::StringSchema::new(#description) }),
            SchemaItem::Object(obj) => {
                let mut elems = TokenStream::new();
                obj.to_schema_inner(&mut elems)?;
                ts.extend(quote! { ::ObjectSchema::new(#description, &[#elems]) })
            }
            SchemaItem::Array(array) => {
                let mut items = TokenStream::new();
                array.to_schema_inner(&mut items)?;
                ts.extend(quote! { ::ArraySchema::new(#description, &#items.schema()) })
            }
        }
        Ok(())
    }
}

/// Contains a sorted list of properties:
struct SchemaObject {
    properties: Vec<(String, bool, Schema)>,
}

impl SchemaObject {
    fn try_extract_from(obj: &mut JSONObject) -> Result<Self, syn::Error> {
        Ok(Self {
            properties: obj
                .remove_required_element("properties")?
                .into_object("object field definition")?
                .into_iter()
                .try_fold(
                    Vec::new(),
                    |mut properties, (key, value)| -> Result<_, syn::Error> {
                        let mut schema: JSONObject =
                            value.into_object("schema definition for field")?;
                        let optional: bool = schema
                            .remove("optional")
                            .map(|opt| -> Result<bool, syn::Error> {
                                let v: syn::LitBool = opt.try_into()?;
                                Ok(v.value)
                            })
                            .transpose()?
                            .unwrap_or(false);
                        properties.push((key.to_string(), optional, schema.try_into()?));
                        Ok(properties)
                    },
                )
                // This must be kept sorted!
                .map(|mut properties| {
                    properties.sort_by(|a, b| (a.0).cmp(&b.0));
                    properties
                })?,
        })
    }

    fn to_schema_inner(&self, ts: &mut TokenStream) -> Result<(), Error> {
        for element in self.properties.iter() {
            let key = &element.0;
            let optional = element.1;
            let mut schema = TokenStream::new();
            element.2.to_schema(&mut schema)?;
            ts.extend(quote! { (#key, #optional, &#schema.schema()), });
        }
        Ok(())
    }
}

struct SchemaArray {
    item: Box<Schema>,
}

impl SchemaArray {
    fn try_extract_from(obj: &mut JSONObject) -> Result<Self, syn::Error> {
        Ok(Self {
            item: Box::new(obj.remove_required_element("items")?.try_into()?),
        })
    }

    fn to_schema_inner(&self, ts: &mut TokenStream) -> Result<(), Error> {
        self.item.to_schema(ts)
    }
}

/// We get macro attributes like `#[input(THIS)]` with the parenthesis around `THIS` included.
struct Parenthesized<T: Parse> {
    pub token: syn::token::Paren,
    pub content: T,
}

impl<T: Parse> Parse for Parenthesized<T> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            token: parenthesized!(content in input),
            content: content.parse()?,
        })
    }
}

/// We get macro attributes like `#[doc = "TEXT"]` with the `=` included.
struct BareAssignment<T: Parse> {
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

/// Parse `#[input()]`, `#[returns()]` and `#[protected]` attributes out of an function annotated
/// with an `#[api]` attribute and produce a `const ApiMethod` named after the function.
///
/// See the top level macro documentation for a complete example.
pub(crate) fn api(_attr: TokenStream, item: TokenStream) -> Result<TokenStream, Error> {
    let mut func: syn::ItemFn = syn::parse2(item)?;

    let sig_span = func.sig.span();

    let mut protected = false;

    let mut input_schema = None;
    let mut returns_schema = None;
    let mut doc_comment = String::new();
    let doc_span = Span::call_site(); // FIXME: set to first doc comment
    for attr in mem::replace(&mut func.attrs, Vec::new()) {
        // don't mess with #![...]
        if let syn::AttrStyle::Inner(_) = &attr.style {
            func.attrs.push(attr);
            continue;
        }

        if attr.path.is_ident("doc") {
            let doc: BareAssignment<syn::LitStr> = syn::parse2(attr.tokens.clone())?;
            if !doc_comment.is_empty() {
                doc_comment.push_str("\n");
            }
            doc_comment.push_str(doc.content.value().trim());
            func.attrs.push(attr);
        } else if attr.path.is_ident("input") {
            let input: Parenthesized<Schema> = syn::parse2(attr.tokens)?;
            input_schema = Some(input.content);
        } else if attr.path.is_ident("returns") {
            let input: Parenthesized<Schema> = syn::parse2(attr.tokens)?;
            returns_schema = Some(input.content);
        } else if attr.path.is_ident("protected") {
            if attr.tokens.is_empty() {
                protected = true;
            } else {
                let value: Parenthesized<syn::LitBool> = syn::parse2(attr.tokens)?;
                protected = value.content.value;
            }
        } else {
            func.attrs.push(attr);
        }
    }

    let mut input_schema =
        input_schema.ok_or_else(|| format_err!(sig_span, "missing input schema"))?;

    let mut returns_schema =
        returns_schema.ok_or_else(|| format_err!(sig_span, "missing returns schema"))?;

    // If we have a doc comment, allow automatically inferring the description for the input and
    // output objects:
    if !doc_comment.is_empty() {
        let mut parts = doc_comment.split("\nReturns:");

        if let Some(first) = parts.next() {
            if input_schema.description.is_none() {
                input_schema.description = Some(syn::LitStr::new(first.trim(), doc_span));
            }
        }

        if let Some(second) = parts.next() {
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

    let input_schema = {
        let mut ts = TokenStream::new();
        input_schema.to_schema(&mut ts)?;
        ts
    };

    let returns_schema = {
        let mut ts = TokenStream::new();
        returns_schema.to_schema(&mut ts)?;
        ts
    };

    let vis = &func.vis;
    let func_name = &func.sig.ident;
    let api_method_name = Ident::new(
        &format!("API_METHOD_{}", func_name.to_string().to_uppercase()),
        func.sig.ident.span(),
    );

    Ok(quote_spanned! { sig_span =>
        #vis const #api_method_name: ::proxmox::api::ApiMethod =
            ::proxmox::api::ApiMethod::new(
                &::proxmox::api::ApiHandler::Sync(&#func_name),
                &#input_schema,
            )
            .returns(& #returns_schema .schema())
            .protected(#protected);
        #func
    })
    //Ok(quote::quote!(#func))
}
