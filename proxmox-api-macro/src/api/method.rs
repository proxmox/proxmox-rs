use std::convert::{TryFrom, TryInto};

use failure::Error;

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::Ident;

use super::{Schema, SchemaItem};
use crate::util::{self, FieldName, JSONObject};

/// Parse `input`, `returns` and `protected` attributes out of an function annotated
/// with an `#[api]` attribute and produce a `const ApiMethod` named after the function.
///
/// See the top level macro documentation for a complete example.
pub fn handle_method(mut attribs: JSONObject, mut func: syn::ItemFn) -> Result<TokenStream, Error> {
    let mut input_schema: Schema = match attribs.remove("input") {
        Some(input) => input.into_object("input schema definition")?.try_into()?,
        None => Schema {
            span: Span::call_site(),
            description: None,
            item: SchemaItem::Object(Default::default()),
            properties: Vec::new(),
        },
    };

    let mut returns_schema: Option<Schema> = attribs
        .remove("returns")
        .map(|ret| ret.into_object("return schema definition"))
        .transpose()?
        .map(|ret| ret.try_into())
        .transpose()?;

    let protected: bool = attribs
        .remove("protected")
        .map(TryFrom::try_from)
        .transpose()?
        .unwrap_or(false);

    let (doc_comment, doc_span) = util::get_doc_comments(&func.attrs)?;
    util::derive_descriptions(
        &mut input_schema,
        &mut returns_schema,
        &doc_comment,
        doc_span,
    )?;

    let mut wrapper_ts = TokenStream::new();
    let is_async = func.sig.asyncness.is_some();
    let api_func_name = handle_function_signature(
        &mut input_schema,
        &mut returns_schema,
        &mut func,
        &mut wrapper_ts,
    )?;

    let input_schema = {
        let mut ts = TokenStream::new();
        input_schema.to_typed_schema(&mut ts)?;
        ts
    };

    let vis = &func.vis;
    let func_name = &func.sig.ident;
    let api_method_name = Ident::new(
        &format!("API_METHOD_{}", func_name.to_string().to_uppercase()),
        func.sig.ident.span(),
    );
    let return_schema_name = Ident::new(
        &format!("API_RETURN_SCHEMA_{}", func_name.to_string().to_uppercase()),
        func.sig.ident.span(),
    );
    let input_schema_name = Ident::new(
        &format!(
            "API_PARAMETER_SCHEMA_{}",
            func_name.to_string().to_uppercase()
        ),
        func.sig.ident.span(),
    );

    let mut returns_schema_definition = TokenStream::new();
    let mut returns_schema_setter = TokenStream::new();
    if let Some(schema) = returns_schema {
        let mut inner = TokenStream::new();
        schema.to_schema(&mut inner)?;
        returns_schema_definition = quote_spanned! { func.sig.span() =>
            pub const #return_schema_name: &'static ::proxmox::api::schema::Schema = (#inner);
        };
        returns_schema_setter = quote! { .returns(#return_schema_name) };
    }

    let api_handler = if is_async {
        quote! { ::proxmox::api::ApiHandler::Async(&#api_func_name) }
    } else {
        quote! { ::proxmox::api::ApiHandler::Sync(&#api_func_name) }
    };

    Ok(quote_spanned! { func.sig.span() =>
        #returns_schema_definition

        pub const #input_schema_name: &'static ::proxmox::api::schema::ObjectSchema =
            (&#input_schema);

        #vis const #api_method_name: ::proxmox::api::ApiMethod =
            ::proxmox::api::ApiMethod::new(
                &#api_handler,
                #input_schema_name,
            )
            #returns_schema_setter
            .protected(#protected);

        #wrapper_ts

        #func
    })
    //Ok(quote::quote!(#func))
}

enum ParameterType<'a> {
    Value,
    ApiMethod,
    RpcEnv,
    Other(&'a syn::Type, bool, &'a Schema),
}

fn check_input_type(input: &syn::FnArg) -> Result<(&syn::PatType, &syn::PatIdent), Error> {
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
    returns_schema: &mut Option<Schema>,
    func: &mut syn::ItemFn,
    wrapper_ts: &mut TokenStream,
) -> Result<Ident, Error> {
    let sig = &func.sig;
    let is_async = sig.asyncness.is_some();

    let mut api_method_param = None;
    let mut rpc_env_param = None;
    let mut value_param = None;

    let mut param_list = Vec::<(FieldName, ParameterType)>::new();

    for input in sig.inputs.iter() {
        let (pat_type, pat) = check_input_type(input)?;

        // For any named type which exists on the function signature...
        if let Some((_ident, optional, ref mut schema)) =
            input_schema.find_obj_property_by_ident_mut(&pat.ident.to_string())
        {
            // try to infer the type in the schema if it is not specified explicitly:
            let is_option = util::infer_type(schema, &*pat_type.ty)?;
            if !is_option && *optional {
                bail!(pat_type => "non-optional `Option` type");
            }
        } else {
            continue;
        };
    }

    for input in sig.inputs.iter() {
        let (pat_type, pat) = check_input_type(input)?;

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
        let mut param_name: FieldName = pat.ident.clone().into();
        let param_type = if let Some((name, optional, schema)) =
            input_schema.find_obj_property_by_ident(&pat.ident.to_string())
        {
            if let SchemaItem::Inferred(span) = &schema.item {
                bail!(*span, "failed to infer type");
            }
            param_name = name.clone();
            // Found an explicit parameter: extract it:
            ParameterType::Other(&pat_type.ty, *optional, schema)
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
        returns_schema,
        param_list,
        func,
        wrapper_ts,
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
    _returns_schema: &Option<Schema>,
    param_list: Vec<(FieldName, ParameterType)>,
    func: &syn::ItemFn,
    wrapper_ts: &mut TokenStream,
    is_async: bool,
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
                let name_str = syn::LitStr::new(name.as_str(), span);
                let arg_name =
                    Ident::new(&format!("input_arg_{}", name.as_ident().to_string()), span);

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
            ::failure::bail!("api function wrapper called with a non-object json value");
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
                //) -> ::std::result::Result<::serde_json::Value, ::failure::Error> {
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
            ) -> ::std::result::Result<::serde_json::Value, ::failure::Error> {
                #body
            }
        });
    }

    Ok(api_func_name)
}
