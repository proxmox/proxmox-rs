extern crate proc_macro;
extern crate proc_macro2;

use std::convert::{TryFrom, TryInto};
use std::mem;

use failure::Error;

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::parse::{Parse, ParseStream, Parser};
use syn::spanned::Spanned;
use syn::Ident;
use syn::{parenthesized, Token};

use crate::util::{JSONObject, JSONValue, SimpleIdent};

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

    fn as_object(&self) -> Option<&SchemaObject> {
        match &self.item {
            SchemaItem::Object(obj) => Some(obj),
            _ => None,
        }
    }

    fn find_object_property(&self, key: &str) -> Option<(bool, &Schema)> {
        self.as_object().and_then(|obj| obj.find_property(key))
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

    fn find_property(&self, key: &str) -> Option<(bool, &Schema)> {
        match self
            .properties
            .binary_search_by(|prope| prope.0.as_str().cmp(key))
        {
            Ok(idx) => Some((self.properties[idx].1, &self.properties[idx].2)),
            Err(_) => None,
        }
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
pub(crate) fn api(attr: TokenStream, item: TokenStream) -> Result<TokenStream, Error> {
    let mut attribs = JSONObject::parse_inner.parse2(attr)?;
    let mut func: syn::ItemFn = syn::parse2(item)?;

    let mut input_schema: Schema = attribs
        .remove_required_element("input")?
        .into_object("input schema definition")?
        .try_into()?;

    let mut returns_schema: Schema = attribs
        .remove_required_element("returns")?
        .into_object("return schema definition")?
        .try_into()?;

    let protected: bool = attribs
        .remove("protected")
        .map(TryFrom::try_from)
        .transpose()?
        .unwrap_or(false);

    api_function_attributes(&mut input_schema, &mut returns_schema, &mut func.attrs)?;

    let mut wrapper_ts = TokenStream::new();
    let api_func_name = handle_function_signature(
        &mut input_schema,
        &mut returns_schema,
        &mut func,
        &mut wrapper_ts,
    )?;

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

    Ok(quote_spanned! { func.sig.span() =>
        #vis const #api_method_name: ::proxmox::api::ApiMethod =
            ::proxmox::api::ApiMethod::new(
                &::proxmox::api::ApiHandler::Sync(&#api_func_name),
                &#input_schema,
            )
            .returns(& #returns_schema .schema())
            .protected(#protected);
        #wrapper_ts
        #func
    })
    //Ok(quote::quote!(#func))
}

fn api_function_attributes(
    input_schema: &mut Schema,
    returns_schema: &mut Schema,
    attrs: &mut Vec<syn::Attribute>,
) -> Result<(), Error> {
    let mut doc_comment = String::new();
    let doc_span = Span::call_site(); // FIXME: set to first doc comment

    for attr in mem::replace(attrs, Vec::new()) {
        // don't mess with #![...]
        if let syn::AttrStyle::Inner(_) = &attr.style {
            attrs.push(attr);
            continue;
        }

        if attr.path.is_ident("doc") {
            let doc: BareAssignment<syn::LitStr> = syn::parse2(attr.tokens.clone())?;
            if !doc_comment.is_empty() {
                doc_comment.push_str("\n");
            }
            doc_comment.push_str(doc.content.value().trim());
            attrs.push(attr);
        } else {
            attrs.push(attr);
        }
    }

    derive_descriptions(input_schema, returns_schema, &doc_comment, doc_span)
}

fn derive_descriptions(
    input_schema: &mut Schema,
    returns_schema: &mut Schema,
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

    Ok(())
}

enum ParameterType<'a> {
    Value,
    ApiMethod,
    RpcEnv,
    Other(&'a syn::Type, bool, &'a Schema),
}

fn handle_function_signature(
    input_schema: &mut Schema,
    returns_schema: &mut Schema,
    func: &mut syn::ItemFn,
    wrapper_ts: &mut TokenStream,
) -> Result<Ident, Error> {
    let sig = &func.sig;

    if sig.asyncness.is_some() {
        bail!(sig => "async fn is currently not supported");
    }

    let mut api_method_param = None;
    let mut rpc_env_param = None;
    let mut value_param = None;

    let mut param_list = Vec::<(SimpleIdent, ParameterType)>::new();

    // Go through the function signature to figure out whether we need to create an internal
    // wrapping function.
    //
    // First: go through the parameters:
    for input in sig.inputs.iter() {
        // `self` types are not supported:
        let pat_type = match input {
            syn::FnArg::Receiver(r) => bail!(r => "methods taking a 'self' are not supported"),
            syn::FnArg::Typed(pat_type) => pat_type,
        };

        // Normally function parameters are simple Ident patterns. Anything else is an error.
        let pat = match &*pat_type.pat {
            syn::Pat::Ident(pat) => pat,
            _ => bail!(pat_type => "unsupported parameter type"),
        };

        // Here's the deal: we need to distinguish between parameters we need to extract before
        // calling the function, a general "Value" parameter covering all the remaining json
        // values, and our 2 fixed function parameters: `&ApiMethod` and `&mut dyn RpcEnvironment`.
        //
        // Our strategy is as follows:
        //     1) See if the parameter name also appears in the input schema. In this case we
        //        assume that we want the parameter to be extracted from the `Value` and passed
        //        directly to the function.
        //
        //     2) Check the parameter type for `&ApiMethod` and remember its position (since we may
        //        need to reorder it!)
        //
        //     3) Check the parameter type for `&dyn RpcEnvironment` and remember its position
        //        (since we may need to reorder it!).
        //
        //     4) Check for a `Value` or `serde_json::Value` parameter. This becomes the
        //        "catch-all" parameter and only 1 may exist.
        //        Note that we may still use further `Value` parameters if they have been
        //        explicitly named in the `input_schema`. However, only 1 unnamed `Value` parameter
        //        is allowed.
        //        If no such parameter exists, we automatically fail the function if the `Value` is
        //        not empty after extracting the parameters.
        //
        //     5) Finally, if none of the above conditions are met, we do not know what to do and
        //        bail out with an error.
        let param_type = if let Some((optional, schema)) =
            input_schema.find_object_property(&pat.ident.to_string())
        {
            // Found an explicit parameter: extract it:
            ParameterType::Other(&pat_type.ty, optional, schema)
        } else if is_api_method_type(&pat_type.ty) {
            if api_method_param.is_some() {
                bail!(pat_type => "multiple ApiMethod parameters found");
            }
            api_method_param = Some(param_list.len());
            ParameterType::ApiMethod
        } else if is_rpc_env_type(&pat_type.ty) {
            if rpc_env_param.is_some() {
                bail!(pat_type => "multiple RpcEnvironment parameters found");
            }
            rpc_env_param = Some(param_list.len());
            ParameterType::RpcEnv
        } else if is_value_type(&pat_type.ty) {
            if value_param.is_some() {
                bail!(pat_type => "multiple additional Value parameters found");
            }
            value_param = Some(param_list.len());
            ParameterType::Value
        } else {
            bail!(&pat.ident => "unexpected parameter");
        };

        param_list.push((pat.ident.clone().into(), param_type));
    }

    // If our function has the correct signature we may not even need a wrapper:
    if (
        param_list.len(),
        value_param,
        api_method_param,
        rpc_env_param,
    ) == (3, Some(0), Some(1), Some(2))
    {
        return Ok(sig.ident.clone());
    }

    create_wrapper_function(input_schema, returns_schema, param_list, func, wrapper_ts)
}

fn is_api_method_type(ty: &syn::Type) -> bool {
    if let syn::Type::Reference(r) = ty {
        if let syn::Type::Path(p) = &*r.elem {
            if p.qself.is_some() {
                return false;
            }
            if let Some(ps) = p.path.segments.last() {
                return ps.ident == "ApiMethod";
            }
        }
    }
    false
}

fn is_rpc_env_type(ty: &syn::Type) -> bool {
    if let syn::Type::Reference(r) = ty {
        if let syn::Type::TraitObject(t) = &*r.elem {
            if let Some(syn::TypeParamBound::Trait(b)) = t.bounds.first() {
                if let Some(ps) = b.path.segments.last() {
                    return ps.ident == "RpcEnvironment";
                }
            }
        }
    }
    false
}

/// Note that we cannot handle renamed imports at all here...
fn is_value_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(p) = ty {
        if p.qself.is_some() {
            return false;
        }
        let segs = &p.path.segments;
        match segs.len() {
            1 => return segs.last().unwrap().ident == "Value",
            2 => {
                return segs.first().unwrap().ident == "serde_json"
                    && segs.last().unwrap().ident == "Value"
            }
            _ => return false,
        }
    }
    false
}

fn create_wrapper_function(
    _input_schema: &Schema,
    _returns_schema: &Schema,
    param_list: Vec<(SimpleIdent, ParameterType)>,
    func: &syn::ItemFn,
    wrapper_ts: &mut TokenStream,
) -> Result<Ident, Error> {
    let api_func_name = Ident::new(
        &format!("api_function_{}", &func.sig.ident),
        func.sig.ident.span(),
    );

    let mut body = TokenStream::new();
    let mut args = TokenStream::new();

    for (name, param) in param_list {
        let span = name.span();
        match param {
            ParameterType::Value => args.extend(quote_spanned! { span => input_params, }),
            ParameterType::ApiMethod => args.extend(quote_spanned! { span => api_method_param, }),
            ParameterType::RpcEnv => args.extend(quote_spanned! { span => rpc_env_param, }),
            ParameterType::Other(_ty, optional, _schema) => {
                let name_str = syn::LitStr::new(&name.to_string(), span);
                let arg_name = Ident::new(&format!("input_arg_{}", name), span);

                // Optional parameters are expected to be Option<> types in the real function
                // signature, so we can just keep the returned Option from `input_map.remove()`.
                body.extend(quote_spanned! { span =>
                    let #arg_name = input_map
                        .remove(#name_str)
                        .map(::serde_json::from_value)
                        .transpose()?
                });
                if !optional {
                    // Non-optional types need to be extracted out of the option though:
                    //
                    // Whether the parameter is optional should have been verified by the schema
                    // verifier already, so here we just use failure::bail! instead of building a
                    // proper http error!
                    body.extend(quote_spanned! { span =>
                        .ok_or_else(|| ::failure::format_err!(
                            "missing non-optional parameter: {}",
                            #name_str,
                        ))?
                    });
                }
                body.extend(quote_spanned! { span => ; });
                args.extend(quote_spanned! { span => #arg_name, });
            }
        }
    }

    // build the wrapping function:
    let func_name = &func.sig.ident;
    wrapper_ts.extend(quote! {
        fn #api_func_name(
            mut input_params: ::serde_json::Value,
            api_method_param: &::proxmox::api::ApiMethod,
            rpc_env_param: &mut dyn ::proxmox::api::RpcEnvironment,
        ) -> Result<::serde_json::Value, ::failure::Error> {
            #[allow(unused_variables)]
            if let Value::Object(ref mut input_map) = &mut input_params {
                #body
                #func_name(#args)
            } else {
                ::failure::bail!("api function wrapper called with a non-object json value");
            }
        }
    });

    return Ok(api_func_name);
}
