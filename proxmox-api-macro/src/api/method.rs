//! Method handling.
//!
//! This has to perform quite a few things: infer types from parameters, deal with optional types
//! and defaults, expose parameter and return value schema to the public, and finally create the
//! wrapper function converting from a json Value hash to the parameters listed in the function
//! signature, while recognizing specially handling `RPCEnvironment` and `ApiMethod` parameters.

use std::convert::{TryFrom, TryInto};
use std::mem;

use anyhow::Error;

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::visit_mut::{self, VisitMut};
use syn::Ident;

use super::{ObjectEntry, Schema, SchemaItem};
use crate::util::{self, FieldName, JSONObject, JSONValue, Maybe};

/// A return type in a schema can have an `optional` flag. Other than that it is just a regular
/// schema.
pub struct ReturnType {
    /// If optional, we store `Some(span)`, otherwise `None`.
    optional: Option<Span>,

    schema: Schema,
}

impl ReturnType {
    fn to_schema(&self, ts: &mut TokenStream) -> Result<(), Error> {
        let optional = match self.optional {
            Some(span) => quote_spanned! { span => true },
            None => quote! { false },
        };

        let mut out = TokenStream::new();
        self.schema.to_schema(&mut out)?;

        ts.extend(quote! {
            ::proxmox::api::router::ReturnType::new( #optional , &#out )
        });
        Ok(())
    }
}

impl TryFrom<JSONValue> for ReturnType {
    type Error = syn::Error;

    fn try_from(value: JSONValue) -> Result<Self, syn::Error> {
        Self::try_from(value.into_object("a return type definition")?)
    }
}

/// To go from a `JSONObject` to a `ReturnType` we first extract the `optional` flag, then forward
/// to the `Schema` parser.
impl TryFrom<JSONObject> for ReturnType {
    type Error = syn::Error;

    fn try_from(mut obj: JSONObject) -> Result<Self, syn::Error> {
        let optional = match obj.remove("optional") {
            Some(value) => {
                let span = value.span();
                let is_optional: bool = value.try_into()?;
                if is_optional {
                    Some(span)
                } else {
                    None
                }
            }
            None => None,
        };

        Ok(Self {
            optional,
            schema: obj.try_into()?,
        })
    }
}

/// Parse `input`, `returns` and `protected` attributes out of an function annotated
/// with an `#[api]` attribute and produce a `const ApiMethod` named after the function.
///
/// See the top level macro documentation for a complete example.
pub fn handle_method(mut attribs: JSONObject, mut func: syn::ItemFn) -> Result<TokenStream, Error> {
    let input_schema: Schema = match attribs.remove("input") {
        Some(input) => input.into_object("input schema definition")?.try_into()?,
        None => Schema {
            span: Span::call_site(),
            description: Maybe::None,
            item: SchemaItem::Object(Default::default()),
            properties: Vec::new(),
        },
    };

    let mut input_schema = if input_schema.as_object().is_some() {
        input_schema
    } else {
        error!(
            input_schema.span,
            "method input schema must be an object schema"
        );
        let mut schema = Schema::empty_object(input_schema.span);
        schema.description = input_schema.description;
        schema.properties = input_schema.properties;
        schema
    };

    let mut return_type: Option<ReturnType> = attribs
        .remove("returns")
        .map(|ret| ret.into_object("return schema definition")?.try_into())
        .transpose()?;

    let access_setter = match attribs.remove("access") {
        Some(access) => {
            let access = Access::try_from(access.into_object("access rules")?)?;
            let permission = access.permission;
            let description = match access.description {
                Some(desc) => quote_spanned! { desc.span() => Some(#desc) },
                None => quote_spanned! { access.span => None },
            };
            quote_spanned! { access.span =>
               .access(#description, #permission)
            }
        }
        None => TokenStream::new(),
    };

    let reload_timezone: bool = attribs
        .remove("reload_timezone")
        .map(TryFrom::try_from)
        .transpose()?
        .unwrap_or(false);

    let protected: bool = attribs
        .remove("protected")
        .map(TryFrom::try_from)
        .transpose()?
        .unwrap_or(false);

    if !attribs.is_empty() {
        error!(
            attribs.span(),
            "unexpected api elements: {}",
            util::join_debug(", ", attribs.elements.keys()),
        );
    }

    let (doc_comment, doc_span) = util::get_doc_comments(&func.attrs)?;
    util::derive_descriptions(
        &mut input_schema,
        return_type.as_mut().map(|rs| &mut rs.schema),
        &doc_comment,
        doc_span,
    )?;

    let mut wrapper_ts = TokenStream::new();
    let mut default_consts = TokenStream::new();

    let is_async = func.sig.asyncness.is_some();
    let api_func_name = handle_function_signature(
        &mut input_schema,
        &mut return_type,
        &mut func,
        &mut wrapper_ts,
        &mut default_consts,
    )?;

    // input schema is done, let's give the method body a chance to extract default parameters:
    DefaultParameters(&input_schema).visit_item_fn_mut(&mut func);

    let vis = &func.vis;
    let func_name = &func.sig.ident;
    let api_method_name = Ident::new(
        &format!("API_METHOD_{}", func_name.to_string().to_uppercase()),
        func.sig.ident.span(),
    );

    let (input_schema_code, input_schema_parameter) =
        serialize_input_schema(input_schema, &func.sig.ident, func.sig.span())?;

    let mut returns_schema_setter = TokenStream::new();
    if let Some(return_type) = return_type {
        let mut inner = TokenStream::new();
        return_type.to_schema(&mut inner)?;
        returns_schema_setter = quote! { .returns(#inner) };
    }

    let api_handler = if is_async {
        quote! { ::proxmox::api::ApiHandler::Async(&#api_func_name) }
    } else {
        quote! { ::proxmox::api::ApiHandler::Sync(&#api_func_name) }
    };

    Ok(quote_spanned! { func.sig.span() =>
        #input_schema_code

        #vis const #api_method_name: ::proxmox::api::ApiMethod =
            ::proxmox::api::ApiMethod::new_full(
                &#api_handler,
                #input_schema_parameter,
            )
            #returns_schema_setter
            #access_setter
            .reload_timezone(#reload_timezone)
            .protected(#protected);

        #default_consts

        #wrapper_ts

        #func
    })
    //Ok(quote::quote!(#func))
}

enum ParameterType<'a> {
    Value,
    ApiMethod,
    RpcEnv,
    Normal(NormalParameter<'a>),
}

struct NormalParameter<'a> {
    ty: &'a syn::Type,
    entry: &'a ObjectEntry,
}

fn check_input_type(input: &syn::FnArg) -> Result<(&syn::PatType, &syn::PatIdent), syn::Error> {
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

    Ok((pat_type, pat))
}

fn handle_function_signature(
    input_schema: &mut Schema,
    return_type: &mut Option<ReturnType>,
    func: &mut syn::ItemFn,
    wrapper_ts: &mut TokenStream,
    default_consts: &mut TokenStream,
) -> Result<Ident, Error> {
    let sig = &func.sig;
    let is_async = sig.asyncness.is_some();

    let mut api_method_param = None;
    let mut rpc_env_param = None;
    let mut value_param = None;

    let mut param_list = Vec::<(FieldName, ParameterType)>::new();

    for input in sig.inputs.iter() {
        let (pat_type, pat) = match check_input_type(input) {
            Ok(input) => input,
            Err(err) => {
                crate::add_error(err);
                continue;
            }
        };

        // For any named type which exists on the function signature...
        if let Some(entry) = input_schema.find_obj_property_by_ident_mut(&pat.ident.to_string()) {
            // try to infer the type in the schema if it is not specified explicitly:
            let is_option = util::infer_type(&mut entry.schema, &*pat_type.ty)?;
            let has_default = entry.schema.find_schema_property("default").is_some();
            if !is_option && entry.optional && !has_default {
                error!(pat_type => "optional types need a default or be an Option<T>");
            }
            if has_default && !entry.optional {
                error!(pat_type => "non-optional parameter cannot have a default");
            }
        } else {
            continue;
        };
    }

    for input in sig.inputs.iter() {
        let (pat_type, pat) = match check_input_type(input) {
            Ok(input) => input,
            Err(_err) => continue, // we already produced errors above,
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
        let pat_ident = pat.ident.unraw();
        let mut param_name: FieldName = pat_ident.clone().into();
        let param_type =
            if let Some(entry) = input_schema.find_obj_property_by_ident(&pat_ident.to_string()) {
                if let SchemaItem::Inferred(span) = &entry.schema.item {
                    bail!(*span, "failed to infer type");
                }
                param_name = entry.name.clone();
                // Found an explicit parameter: extract it:
                ParameterType::Normal(NormalParameter {
                    ty: &pat_type.ty,
                    entry: &entry,
                })
            } else if is_api_method_type(&pat_type.ty) {
                if api_method_param.is_some() {
                    error!(pat_type => "multiple ApiMethod parameters found");
                    continue;
                }
                api_method_param = Some(param_list.len());
                ParameterType::ApiMethod
            } else if is_rpc_env_type(&pat_type.ty) {
                if rpc_env_param.is_some() {
                    error!(pat_type => "multiple RpcEnvironment parameters found");
                    continue;
                }
                rpc_env_param = Some(param_list.len());
                ParameterType::RpcEnv
            } else if is_value_type(&pat_type.ty) {
                if value_param.is_some() {
                    error!(pat_type => "multiple additional Value parameters found");
                    continue;
                }
                value_param = Some(param_list.len());
                ParameterType::Value
            } else {
                error!(&pat_ident => "unexpected parameter {:?}", pat_ident.to_string());
                continue;
            };

        param_list.push((param_name, param_type));
    }

    /*
     * Doing this is actually unreliable, since we cannot support aliased Result types, or all
     * poassible combinations of paths like `result::Result<>` or `std::result::Result<>` or
     * `ApiResult`.

    // Secondly, take a look at the return type, and then decide what to do:
    // If our function has the correct signature we may not even need a wrapper.
    if is_default_return_type(&sig.output)
        && (
            param_list.len(),
            value_param,
            api_method_param,
            rpc_env_param,
        ) == (3, Some(0), Some(1), Some(2))
    {
        return Ok(sig.ident.clone());
    }
    */

    create_wrapper_function(
        input_schema,
        return_type,
        param_list,
        func,
        wrapper_ts,
        default_consts,
        is_async,
    )
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
    _returns_schema: &Option<ReturnType>,
    param_list: Vec<(FieldName, ParameterType)>,
    func: &syn::ItemFn,
    wrapper_ts: &mut TokenStream,
    default_consts: &mut TokenStream,
    is_async: bool,
) -> Result<Ident, Error> {
    let api_func_name = Ident::new(
        &format!("api_function_{}", &func.sig.ident),
        func.sig.ident.span(),
    );

    let mut body = TokenStream::new();
    let mut args = TokenStream::new();

    let func_uc = func.sig.ident.to_string().to_uppercase();

    for (name, param) in param_list {
        let span = name.span();
        match param {
            ParameterType::Value => args.extend(quote_spanned! { span => input_params, }),
            ParameterType::ApiMethod => args.extend(quote_spanned! { span => api_method_param, }),
            ParameterType::RpcEnv => args.extend(quote_spanned! { span => rpc_env_param, }),
            ParameterType::Normal(param) => {
                extract_normal_parameter(
                    param,
                    &mut body,
                    &mut args,
                    &func_uc,
                    name,
                    span,
                    default_consts,
                )?;
            }
        }
    }

    // build the wrapping function:
    let func_name = &func.sig.ident;

    let await_keyword = if is_async { Some(quote!(.await)) } else { None };

    let question_mark = match func.sig.output {
        syn::ReturnType::Default => None,
        _ => Some(quote!(?)),
    };

    let body = quote! {
        if let ::serde_json::Value::Object(ref mut input_map) = &mut input_params {
            #body
            Ok(::serde_json::to_value(#func_name(#args) #await_keyword #question_mark)?)
        } else {
            ::anyhow::bail!("api function wrapper called with a non-object json value");
        }
    };

    if is_async {
        wrapper_ts.extend(quote! {
            fn #api_func_name<'a>(
                mut input_params: ::serde_json::Value,
                api_method_param: &'static ::proxmox::api::ApiMethod,
                rpc_env_param: &'a mut dyn ::proxmox::api::RpcEnvironment,
            ) -> ::proxmox::api::ApiFuture<'a> {
                //async fn func<'a>(
                //    mut input_params: ::serde_json::Value,
                //    api_method_param: &'static ::proxmox::api::ApiMethod,
                //    rpc_env_param: &'a mut dyn ::proxmox::api::RpcEnvironment,
                //) -> ::std::result::Result<::serde_json::Value, ::anyhow::Error> {
                //    #body
                //}
                //::std::boxed::Box::pin(async move {
                //    func(input_params, api_method_param, rpc_env_param).await
                //})
                ::std::boxed::Box::pin(async move { #body })
            }
        });
    } else {
        wrapper_ts.extend(quote! {
            fn #api_func_name(
                mut input_params: ::serde_json::Value,
                api_method_param: &::proxmox::api::ApiMethod,
                rpc_env_param: &mut dyn ::proxmox::api::RpcEnvironment,
            ) -> ::std::result::Result<::serde_json::Value, ::anyhow::Error> {
                #body
            }
        });
    }

    Ok(api_func_name)
}

fn extract_normal_parameter(
    param: NormalParameter,
    body: &mut TokenStream,
    args: &mut TokenStream,
    func_uc: &str,
    name: FieldName,
    name_span: Span,
    default_consts: &mut TokenStream,
) -> Result<(), Error> {
    let span = name_span; // renamed during refactorization
    let name_str = syn::LitStr::new(name.as_str(), span);
    let arg_name = Ident::new(&format!("input_arg_{}", name.as_ident().to_string()), span);

    let default_value = param.entry.schema.find_schema_property("default");

    // Optional parameters are expected to be Option<> types in the real function
    // signature, so we can just keep the returned Option from `input_map.remove()`.
    match param.entry.flatten {
        None => {
            // regular parameter, we just remove it and call `from_value`.

            body.extend(quote_spanned! { span =>
                let #arg_name = input_map
                    .remove(#name_str)
                    .map(::serde_json::from_value)
                    .transpose()?
            });

            if !param.entry.optional {
                // Non-optional types need to be extracted out of the option though (unless
                // they have a default):
                //
                // Whether the parameter is optional should have been verified by the schema
                // verifier already, so here we just use anyhow::bail! instead of building a
                // proper http error!
                body.extend(quote_spanned! { span =>
                    .ok_or_else(|| ::anyhow::format_err!(
                        "missing non-optional parameter: {}",
                        #name_str,
                    ))?
                });
            }

            let no_option_type = util::is_option_type(param.ty).is_none();

            if let Some(def) = &default_value {
                let name_uc = name.as_ident().to_string().to_uppercase();
                let name = Ident::new(
                    &format!("API_METHOD_{}_PARAM_DEFAULT_{}", func_uc, name_uc),
                    span,
                );

                // strip possible Option<> from this type:
                let ty = util::is_option_type(param.ty).unwrap_or(param.ty);
                default_consts.extend(quote_spanned! { span =>
                    pub const #name: #ty = #def;
                });

                if param.entry.optional && no_option_type {
                    // Optional parameter without an Option<T> type requires a default:
                    body.extend(quote_spanned! { span =>
                        .unwrap_or(#name)
                    });
                }
            } else if param.entry.optional && no_option_type {
                // FIXME: we should not be able to reach this without having produced another
                // error above already anyway?
                error!(param.ty => "Optional parameter without Option<T> requires a default");

                // we produced an error so just write something that will compile
                body.extend(quote_spanned! { span =>
                    .unwrap_or_else(|| unreachable!())
                });
            }

            body.extend(quote_spanned! { span => ; });
        }
        Some(flatten_span) => {
            // Flattened parameter, we need ot use our special partial-object deserializer.
            // Also note that we do not support simply nesting schemas. We need a referenced type.
            // Otherwise the expanded code here gets ugly and we'd need to make sure we pull out
            // nested schemas into named variables first... No thanks.

            if default_value.is_some() {
                error!(
                    default_value =>
                    "flattened parameter cannot have a default as it cannot be optional",
                );
            }

            if let Some(schema_ref) = param.entry.schema.to_schema_reference() {
                let ty = param.ty;
                body.extend(quote_spanned! { span =>
                    let #arg_name = <#ty as ::serde::Deserialize>::deserialize(
                        ::proxmox::api::de::ExtractValueDeserializer::try_new(
                            input_map,
                            #schema_ref,
                        )
                        .ok_or_else(|| ::anyhow::format_err!(
                            "flattened parameter {:?} has invalid schema",
                            #name_str,
                        ))?,
                    )?;
                });
            } else {
                error!(
                    flatten_span,
                    "flattened parameter schema must be a schema reference"
                );
                body.extend(quote_spanned! { span => let #arg_name = unreachable!(); });
            }
        }
    }

    args.extend(quote_spanned! { span => #arg_name, });

    Ok(())
}

/// Returns a tuple containing the schema code first and the `ParameterSchema` parameter for the
/// `ApiMethod` second.
fn serialize_input_schema(
    mut input_schema: Schema,
    func_name: &Ident,
    func_sig_span: Span,
) -> Result<(TokenStream, TokenStream), Error> {
    let input_schema_name = Ident::new(
        &format!(
            "API_PARAMETER_SCHEMA_{}",
            func_name.to_string().to_uppercase()
        ),
        func_name.span(),
    );

    let (flattened, has_params) = match &mut input_schema.item {
        SchemaItem::Object(obj) => {
            let flattened = obj.drain_filter(|entry| entry.flatten.is_none());
            (flattened, !obj.is_empty())
        }
        _ => (Vec::new(), true),
    };

    if flattened.is_empty() {
        let mut ts = TokenStream::new();
        input_schema.to_typed_schema(&mut ts)?;
        return Ok((
            quote_spanned! { func_sig_span =>
                pub const #input_schema_name: ::proxmox::api::schema::ObjectSchema = #ts;
            },
            quote_spanned! { func_sig_span =>
                ::proxmox::api::router::ParameterSchema::Object(&#input_schema_name)
            },
        ));
    }

    let mut all_of_schemas = TokenStream::new();
    for entry in flattened {
        if entry.optional {
            error!(
                entry.schema.span,
                "optional flattened parameters are not supported"
            );
        }

        all_of_schemas.extend(quote::quote! {&});
        entry.schema.to_schema(&mut all_of_schemas)?;
        all_of_schemas.extend(quote::quote! {,});
    }

    let description = match input_schema.description.take().ok() {
        Some(description) => description,
        None => {
            error!(input_schema.span, "missing description on api type struct");
            syn::LitStr::new("<missing description>", input_schema.span)
        }
    };
    // and replace it with a "dummy"
    input_schema.description = Maybe::Derived(syn::LitStr::new(
        &format!("<INNER: {}>", description.value()),
        description.span(),
    ));

    let (inner_schema, inner_schema_ref) = if has_params {
        // regular parameters go into the "inner" schema to merge into the AllOfSchema
        let inner_schema_name = Ident::new(
            &format!(
                "API_REGULAR_PARAMETER_SCHEMA_{}",
                func_name.to_string().to_uppercase()
            ),
            func_name.span(),
        );

        let obj_schema = {
            let mut ts = TokenStream::new();
            input_schema.to_schema(&mut ts)?;
            ts
        };

        (
            quote_spanned!(func_sig_span =>
                const #inner_schema_name: ::proxmox::api::schema::Schema = #obj_schema;
            ),
            quote_spanned!(func_sig_span => &#inner_schema_name,),
        )
    } else {
        // otherwise it stays empty
        (TokenStream::new(), TokenStream::new())
    };

    Ok((
        quote_spanned! { func_sig_span =>
            #inner_schema

            pub const #input_schema_name: ::proxmox::api::schema::AllOfSchema =
                ::proxmox::api::schema::AllOfSchema::new(
                    #description,
                    &[
                        #inner_schema_ref
                        #all_of_schemas
                    ],
                );
        },
        quote_spanned! { func_sig_span =>
            ::proxmox::api::router::ParameterSchema::AllOf(&#input_schema_name)
        },
    ))
}

struct DefaultParameters<'a>(&'a Schema);

impl<'a> VisitMut for DefaultParameters<'a> {
    fn visit_expr_mut(&mut self, i: &mut syn::Expr) {
        if let syn::Expr::Macro(exprmac) = i {
            if exprmac.mac.path.is_ident("api_get_default") {
                // replace api_get_default macros with the actual default found in the #[api]
                // macro.
                match self.get_default(mem::take(&mut exprmac.mac.tokens)) {
                    Ok(expr) => *i = expr,
                    Err(err) => {
                        *i = syn::Expr::Verbatim(err.to_compile_error());
                        return;
                    }
                }
            }
        }

        visit_mut::visit_expr_mut(self, i)
    }
}

impl<'a> DefaultParameters<'a> {
    fn get_default(&self, param_tokens: TokenStream) -> Result<syn::Expr, syn::Error> {
        let param_name: syn::LitStr = syn::parse2(param_tokens)?;
        match self.0.find_obj_property_by_ident(&param_name.value()) {
            Some(entry) => match entry.schema.find_schema_property("default") {
                Some(def) => Ok(def.clone()),
                None => bail!(param_name => "no default found in schema"),
            },
            None => bail!(param_name => "todo"),
        }
    }
}

struct Access {
    span: Span,
    description: Option<syn::LitStr>,
    permission: syn::Expr,
}

impl TryFrom<JSONValue> for Access {
    type Error = syn::Error;

    fn try_from(value: JSONValue) -> Result<Self, syn::Error> {
        Self::try_from(value.into_object("an access definition")?)
    }
}

impl TryFrom<JSONObject> for Access {
    type Error = syn::Error;

    fn try_from(mut obj: JSONObject) -> Result<Self, syn::Error> {
        let description = match obj.remove("description") {
            Some(v) => Some(v.try_into()?),
            None => None,
        };

        let permission = obj
            .remove("permission")
            .ok_or_else(|| format_err!(obj.span(), "missing `permission` field"))?
            .try_into()?;

        if !obj.is_empty() {
            bail!(
                obj.span(),
                "unexpected elements: {}",
                util::join_debug(", ", obj.elements.keys()),
            );
        }

        Ok(Self {
            span: obj.span(),
            description,
            permission,
        })
    }
}
