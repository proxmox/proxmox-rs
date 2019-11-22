extern crate proc_macro;
extern crate proc_macro2;

use std::mem;

use failure::Error;

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::Ident;
use syn::{parenthesized, Token};

/// Any 'keywords' we introduce as part of our schema related api macro syntax.
mod token {
    syn::custom_keyword!(optional);
}

/// Our syntax elements which represent an API Schema implement this. This is similar to
/// `quote::ToTokens`, but rather than translating back into the input, this produces the resulting
/// `proxmox::api::schema::Schema` instantiation.
///
/// For example:
/// ```ignore
/// Schema {
///     item_type: "Boolean",
///     paren_token: ...,
///     description: Some("Some value"),
///     comma_token: ...,
///     item: SchemaItem::Boolean(SchemaItemBoolean {
///         default_value: Some(DefaultValue {
///             default_token: ...,
///             colon: ...,
///             value: syn::ExprLit(syn::LitBool(true)), // simplified...
///         }),
///     }),
///     constraints: Vec::new(),
/// }.to_schema(ts);
/// ```
///
/// produces:
///
/// ```ignore
/// ::proxmox::api::schema::BooleanSchema::new("Some value")
///     .default(true)
/// ```
trait ToSchema {
    fn to_schema(&self, ts: &mut TokenStream) -> Result<(), Error>;

    #[inline]
    fn add_constraints(&self, ts: &mut TokenStream) -> Result<(), Error> {
        let _ = ts;
        Ok(())
    }
}

/// A generic schema entry.
///
/// Since all our schema types have at least a description, we define this "top level" schema
/// syntax element which parses the description as first parameter (if it is available), and then
/// parses the remaining parts as `SchemaItem`.
///
/// ```text
/// Object    ( "Description", { Elements } ) .default_key("hello")
/// ^^^^^^    ~ ^^^^^^^^^^^^^^ ~~~~~~~~~~~~ ^ ~~~~~~~~~~~~~~~~~~~~~
/// item_type   description    item           constraints
/// ```
struct Schema {
    pub item_type: Ident,
    pub paren_token: syn::token::Paren,
    pub description: Option<syn::LitStr>,
    pub comma_token: Option<Token![,]>,
    pub item: SchemaItem,
    pub constraints: Vec<syn::ExprCall>,
}

impl ToSchema for Schema {
    fn to_schema(&self, ts: &mut TokenStream) -> Result<(), Error> {
        let item_type = &self.item_type;
        let schema_type = Ident::new(
            &format!("{}Schema", item_type.to_string()),
            item_type.span(),
        );
        let description = self
            .description
            .as_ref()
            .ok_or_else(|| format_err!(item_type => "missing description"))?;

        let mut item = TokenStream::new();
        self.item.to_schema(&mut item)?;

        ts.extend(quote! {
            ::proxmox::api::schema::#schema_type::new(
                #description,
                #item
            )
        });
        self.item.add_constraints(ts)?;

        for constraint in self.constraints.iter() {
            ts.extend(quote! { . #constraint });
        }

        Ok(())
    }
}

impl Parse for Schema {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let item_type: Ident = input.parse()?;
        let item_type_span = item_type.span();
        let item_type_str = item_type.to_string();
        let content;
        let mut comma_token = None;
        Ok(Self {
            item_type,
            paren_token: parenthesized!(content in input),
            description: {
                let lookahead = content.lookahead1();
                if lookahead.peek(syn::LitStr) {
                    let desc = content.parse()?;
                    if !content.is_empty() {
                        comma_token = Some(content.parse()?);
                    }
                    Some(desc)
                } else {
                    None
                }
            },
            comma_token,
            item: {
                match item_type_str.as_str() {
                    "Null" => content.parse().map(SchemaItem::Null)?,
                    "Boolean" => content.parse().map(SchemaItem::Boolean)?,
                    "Integer" => content.parse().map(SchemaItem::Integer)?,
                    "String" => content.parse().map(SchemaItem::String)?,
                    "Object" => content.parse().map(SchemaItem::Object)?,
                    "Array" => content.parse().map(SchemaItem::Array)?,
                    _ => bail!(item_type_span, "unknown schema type"),
                }
            },
            constraints: {
                let mut constraints = Vec::<syn::ExprCall>::new();
                while input.lookahead1().peek(Token![.]) {
                    let _dot: Token![.] = input.parse()?;
                    constraints.push(input.parse()?);
                }
                constraints
            },
        })
    }
}

/// This is the collection of possible schema elements we have.
///
/// Its `ToSchema` implementation simply defers to the inner types. It has no `Parse`
/// implementation directly. This is handled by the parser for `Schema`.
enum SchemaItem {
    Null(SchemaItemNull),
    Boolean(SchemaItemBoolean),
    Integer(SchemaItemInteger),
    String(SchemaItemString),
    Object(SchemaItemObject),
    Array(SchemaItemArray),
}

impl ToSchema for SchemaItem {
    fn to_schema(&self, ts: &mut TokenStream) -> Result<(), Error> {
        match self {
            SchemaItem::Null(i) => i.to_schema(ts),
            SchemaItem::Boolean(i) => i.to_schema(ts),
            SchemaItem::Integer(i) => i.to_schema(ts),
            SchemaItem::String(i) => i.to_schema(ts),
            SchemaItem::Object(i) => i.to_schema(ts),
            SchemaItem::Array(i) => i.to_schema(ts),
        }
    }

    #[inline]
    fn add_constraints(&self, ts: &mut TokenStream) -> Result<(), Error> {
        match self {
            SchemaItem::Null(i) => i.add_constraints(ts),
            SchemaItem::Boolean(i) => i.add_constraints(ts),
            SchemaItem::Integer(i) => i.add_constraints(ts),
            SchemaItem::String(i) => i.add_constraints(ts),
            SchemaItem::Object(i) => i.add_constraints(ts),
            SchemaItem::Array(i) => i.add_constraints(ts),
        }
    }
}

/// A "default key" for an object schema.
///
/// This serves mostly as an example of how we could extend the macro syntax.
/// This is used typing the following:
///
/// ```ignore
/// Object("Description", default: "foo", { "foo": String("Foo"), "bar": String("Bar") })
/// ```
///
/// instead of:
///
/// ```ignore
/// Object("Description", { "foo": String("Foo"), "bar": String("Bar") }).default_key("foo")
/// ```
struct DefaultKey {
    pub default_token: Token![default],
    pub colon: Token![:],
    pub key_name: syn::LitStr,
    pub comma_token: Token![,],
}

impl Parse for DefaultKey {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            default_token: input.parse()?,
            colon: input.parse()?,
            key_name: input.parse()?,
            comma_token: input.parse()?,
        })
    }
}

/// An object schema. This currently allows parsing a default key as an example of what we could do
/// instead of keeping the builder-pattern syntax within the macro invocation.
///
/// The elements then follow enclosed in braces:
///
/// ```ignore
/// Object("Description", { "key1": Integer("Key One"), optional "key2": Integer("Key Two") })
/// ```
struct SchemaItemObject {
    pub default_key: Option<DefaultKey>,
    pub brace_token: syn::token::Brace,
    pub elements: Punctuated<ObjectElement, Token![,]>,
}

impl ToSchema for SchemaItemObject {
    fn to_schema(&self, ts: &mut TokenStream) -> Result<(), Error> {
        let mut elements: Vec<&ObjectElement> = self.elements.iter().collect();
        elements.sort_by(|a, b| a.cmp(b));

        let mut elem_ts = TokenStream::new();
        for element in elements {
            if !elem_ts.is_empty() {
                elem_ts.extend(quote![, ]);
            }

            element.to_schema(&mut elem_ts)?;
        }

        ts.extend(quote! { & [ #elem_ts ] });

        Ok(())
    }

    fn add_constraints(&self, ts: &mut TokenStream) -> Result<(), Error> {
        if let Some(def) = &self.default_key {
            let key = &def.key_name;
            ts.extend(quote! { .default_key(#key) });
        }
        Ok(())
    }
}

impl Parse for SchemaItemObject {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let elements;
        Ok(Self {
            default_key: {
                let lookahead = input.lookahead1();
                if lookahead.peek(Token![default]) {
                    Some(input.parse()?)
                } else {
                    None
                }
            },
            brace_token: syn::braced!(elements in input),
            elements: elements.parse_terminated(ObjectElement::parse)?,
        })
    }
}

/// This represents a member in the comma separated list of fields of an object.
///
/// ```text
/// Object("Description", { "key1": Integer("Key One"), optional "key2": Integer("Key Two") })
///                         ^^^^^^^^^^^^^^^^^^^^^^^^^^  ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
///                         one `ObjectElement`         another `ObjectElement`
/// ```
struct ObjectElement {
    pub optional: Option<token::optional>,
    pub field_name: syn::LitStr,
    pub colon: Token![:],
    pub item: Schema,
}

impl ObjectElement {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.field_name.suffix().cmp(other.field_name.suffix())
    }
}

impl ToSchema for ObjectElement {
    fn to_schema(&self, ts: &mut TokenStream) -> Result<(), Error> {
        let mut schema = TokenStream::new();
        self.item.to_schema(&mut schema)?;

        let name = &self.field_name;

        let optional = if self.optional.is_some() {
            quote!(true)
        } else {
            quote!(false)
        };

        ts.extend(quote! {
            (#name, #optional, & #schema .schema())
        });

        Ok(())
    }
}

impl Parse for ObjectElement {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            optional: input.parse()?,
            field_name: input.parse()?,
            colon: input.parse()?,
            item: input.parse()?,
        })
    }
}

/// Array schemas simply contain their inner type.
///
/// ```ignore
/// Array("Some data", Integer("A data element"))
/// ```
struct SchemaItemArray {
    pub item_schema: Box<Schema>,
}

impl ToSchema for SchemaItemArray {
    fn to_schema(&self, ts: &mut TokenStream) -> Result<(), Error> {
        ts.extend(quote! { & });
        self.item_schema.to_schema(ts)?;
        self.item_schema.add_constraints(ts)?;
        ts.extend(quote! { .schema() });
        Ok(())
    }
}

impl Parse for SchemaItemArray {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            item_schema: Box::new(input.parse()?),
        })
    }
}

/// The `Null` schema.
struct SchemaItemNull {}

impl ToSchema for SchemaItemNull {
    fn to_schema(&self, _ts: &mut TokenStream) -> Result<(), Error> {
        Ok(())
    }
}

impl Parse for SchemaItemNull {
    fn parse(_input: ParseStream) -> syn::Result<Self> {
        Ok(Self {})
    }
}

/// A default value. Similar to the default keys in objects, this is an example of a different
/// syntax instead of the builder pattern.
///
/// ```ignore
/// String("Something", default: "The default value")
/// ```
///
/// instead of:
///
/// ```ignore
/// String("Something").default("The default value")
/// ```
struct DefaultValue {
    pub default_token: Token![default],
    pub colon: Token![:],
    pub value: syn::Expr,
}

impl ToSchema for DefaultValue {
    fn to_schema(&self, _ts: &mut TokenStream) -> Result<(), Error> {
        Ok(())
    }

    fn add_constraints(&self, ts: &mut TokenStream) -> Result<(), Error> {
        let value = &self.value;
        ts.extend(quote! { .default(#value) });
        Ok(())
    }
}

impl Parse for DefaultValue {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            default_token: input.parse()?,
            colon: input.parse()?,
            value: input.parse()?,
        })
    }
}

macro_rules! try_parse_default_value {
    ($input:expr) => {{
        let input = $input;
        let lookahead = input.lookahead1();
        if lookahead.peek(Token![default]) {
            Some(input.parse()?)
        } else {
            None
        }
    }};
}

/// A boolean schema entry.
struct SchemaItemBoolean {
    pub default_value: Option<DefaultValue>,
}

impl ToSchema for SchemaItemBoolean {
    fn to_schema(&self, _ts: &mut TokenStream) -> Result<(), Error> {
        Ok(())
    }

    fn add_constraints(&self, ts: &mut TokenStream) -> Result<(), Error> {
        if let Some(def) = &self.default_value {
            def.add_constraints(ts)?;
        }
        Ok(())
    }
}

impl Parse for SchemaItemBoolean {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            default_value: try_parse_default_value!(input),
        })
    }
}

/// An integer schema entry.
struct SchemaItemInteger {
    pub default_value: Option<DefaultValue>,
}

impl ToSchema for SchemaItemInteger {
    fn to_schema(&self, _ts: &mut TokenStream) -> Result<(), Error> {
        Ok(())
    }

    fn add_constraints(&self, ts: &mut TokenStream) -> Result<(), Error> {
        if let Some(def) = &self.default_value {
            def.add_constraints(ts)?;
        }
        Ok(())
    }
}

impl Parse for SchemaItemInteger {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            default_value: try_parse_default_value!(input),
        })
    }
}

/// An string schema entry.
struct SchemaItemString {
    pub default_value: Option<DefaultValue>,
}

impl ToSchema for SchemaItemString {
    fn to_schema(&self, _ts: &mut TokenStream) -> Result<(), Error> {
        Ok(())
    }

    fn add_constraints(&self, ts: &mut TokenStream) -> Result<(), Error> {
        if let Some(def) = &self.default_value {
            def.add_constraints(ts)?;
        }
        Ok(())
    }
}

impl Parse for SchemaItemString {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            default_value: try_parse_default_value!(input),
        })
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
            doc_comment.push_str(&doc.content.value());
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

    if input_schema.description.is_none() {
        input_schema.description = Some(syn::LitStr::new(&doc_comment, doc_span));
    }

    let input_schema = {
        let mut ts = TokenStream::new();
        input_schema.to_schema(&mut ts)?;
        ts
    };

    let returns_schema =
        returns_schema.ok_or_else(|| format_err!(sig_span, "missing returns schema"))?;

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
