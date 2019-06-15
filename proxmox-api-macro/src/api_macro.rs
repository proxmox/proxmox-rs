use std::collections::HashMap;

use proc_macro2::{Delimiter, Ident, Span, TokenStream, TokenTree};

use failure::{bail, format_err, Error};
use quote::{quote, ToTokens};
use syn::{Expr, Token};

use super::api_def::ParameterDefinition;
use super::parsing::*;

pub fn api_macro(attr: TokenStream, item: TokenStream) -> Result<TokenStream, Error> {
    let definition = attr
        .into_iter()
        .next()
        .expect("expected api definition in braces");

    let definition = match definition {
        TokenTree::Group(ref group) if group.delimiter() == Delimiter::Brace => group.stream(),
        _ => bail!("expected api definition in braces"),
    };

    let definition = parse_object2(definition)?;

    // Now parse the item, based on which we decide whether this is an API method which needs a
    // wrapper, or an API type which needs an ApiType implementation!
    let item: syn::Item = syn::parse2(item).unwrap();

    match item {
        syn::Item::Struct(ref itemstruct) => {
            let extra = handle_struct(definition, itemstruct)?;
            let mut output = item.into_token_stream();
            output.extend(extra);
            Ok(output)
        }
        syn::Item::Fn(func) => handle_function(definition, func),
        _ => bail!("api macro currently only applies to structs and functions"),
    }
}

fn handle_function(
    mut definition: HashMap<String, Expression>,
    mut item: syn::ItemFn,
) -> Result<TokenStream, Error> {
    if item.decl.generics.lt_token.is_some() {
        bail!("cannot use generic functions for api macros currently");
        // Not until we stabilize our generated representation!
    }

    // We cannot use #{foo.bar} in quote!, we can only use #foo, so these must all be local
    // variables. (I'd prefer a struct and using `#{func.description}`, `#{func.protected}` etc.
    // but that's not supported.

    let fn_api_description = definition
        .remove("description")
        .ok_or_else(|| format_err!("missing 'description' in method definition"))?
        .expect_lit_str()?;

    let fn_api_protected = definition
        .remove("protected")
        .map(|v| v.expect_lit_bool())
        .transpose()?
        .unwrap_or_else(|| syn::LitBool {
            span: Span::call_site(),
            value: false,
        });

    let fn_api_reload_timezone = definition
        .remove("reload-timezone")
        .map(|v| v.expect_lit_bool())
        .transpose()?
        .unwrap_or_else(|| syn::LitBool {
            span: Span::call_site(),
            value: false,
        });

    let body_type = definition
        .remove("body")
        .map(|v| v.expect_type())
        .transpose()?
        .map_or_else(|| quote! { ::hyper::Body }, |v| v.into_token_stream());

    let mut parameters = definition
        .remove("parameters")
        .map(|v| v.expect_object())
        .transpose()?
        .unwrap_or_else(HashMap::new);
    let mut parameter_entries = TokenStream::new();
    let mut parameter_verifiers = TokenStream::new();

    let vis = std::mem::replace(&mut item.vis, syn::Visibility::Inherited);
    let span = item.ident.span();
    let name_str = item.ident.to_string();
    //let impl_str = format!("{}_impl", name_str);
    //let impl_ident = Ident::new(&impl_str, span);
    let impl_checked_str = format!("{}_checked_impl", name_str);
    let impl_checked_ident = Ident::new(&impl_checked_str, span);
    let impl_unchecked_str = format!("{}_unchecked_impl", name_str);
    let impl_unchecked_ident = Ident::new(&impl_unchecked_str, span);
    let name = std::mem::replace(&mut item.ident, impl_unchecked_ident.clone());
    let mut return_type = match item.decl.output {
        syn::ReturnType::Default => syn::Type::Tuple(syn::TypeTuple {
            paren_token: syn::token::Paren {
                span: Span::call_site(),
            },
            elems: syn::punctuated::Punctuated::new(),
        }),
        syn::ReturnType::Type(_, ref ty) => ty.as_ref().clone(),
    };

    let mut extracted_args = syn::punctuated::Punctuated::<Ident, Token![,]>::new();
    let mut passed_args = syn::punctuated::Punctuated::<Ident, Token![,]>::new();
    let mut arg_extraction = Vec::new();

    let inputs = item.decl.inputs.clone();
    for arg in item.decl.inputs.iter() {
        let arg = match arg {
            syn::FnArg::Captured(ref arg) => arg,
            other => bail!("unhandled type of method parameter ({:?})", other),
        };

        let arg_type = &arg.ty;
        let name = match &arg.pat {
            syn::Pat::Ident(name) => &name.ident,
            other => bail!("invalid kind of parameter pattern: {:?}", other),
        };
        passed_args.push(name.clone());
        let name_str = name.to_string();

        let arg_name = Ident::new(&format!("arg_{}", name_str), name.span());
        extracted_args.push(arg_name.clone());

        arg_extraction.push(quote! {
            let #arg_name = ::serde_json::from_value(
                args
                    .remove(#name_str)
                    .unwrap_or(::serde_json::Value::Null)
            )?;
        });

        let info = parameters
            .remove(&name_str)
            .ok_or_else(|| format_err!("missing parameter '{}' in api defintion", name_str))?;

        match info {
            Expression::Expr(Expr::Lit(lit)) => {
                parameter_entries.extend(quote! {
                    ::proxmox::api::Parameter {
                        name: #name_str,
                        description: #lit,
                        type_info: <#arg_type as ::proxmox::api::ApiType>::type_info,
                    },
                });
            }
            Expression::Expr(_) => bail!("description must be a string literal!"),
            Expression::Object(mut param_info) => {
                let description = param_info
                    .remove("description")
                    .ok_or_else(|| format_err!("missing 'description' in parameter definition"))?
                    .expect_lit_str()?;

                parameter_entries.extend(quote! {
                    ::proxmox::api::Parameter {
                        name: #name_str,
                        description: #description,
                        type_info: <#arg_type as ::proxmox::api::ApiType>::type_info,
                    },
                });

                make_parameter_verifier(
                    &name,
                    &name_str,
                    &mut param_info,
                    &mut parameter_verifiers,
                )?;
            }
        }
    }

    if !parameters.is_empty() {
        let mut list = String::new();
        for param in parameters.keys() {
            if !list.is_empty() {
                list.push_str(", ");
            }
            list.push_str(&param);
        }
        bail!(
            "api definition contains parameters not found in function declaration: {}",
            list
        );
    }

    use std::iter::FromIterator;
    let arg_extraction = TokenStream::from_iter(arg_extraction.into_iter());

    // The router expects an ApiMethod, or more accurately, an object implementing ApiMethodInfo.
    // This is because we need access to a bunch of additional attributes of the functions both at
    // runtime and when doing command line parsing/completion/help output.
    //
    // When manually implementing methods, we usually just write them out as an `ApiMethod` which
    // is a type requiring all the info made available by the ApiMethodInfo trait as members.
    //
    // While we could just generate a `const ApiMethod` for our functions, we would like them to
    // also be usable as functions simply because the syntax we use to create them makes them
    // *look* like functions, so it would be nice if they also *behaved* like real functions.
    //
    // Therefore all the fields of an ApiMethod are accessed via methods from the ApiMethodInfo
    // trait and we perform the same trick lazy_static does: Create a new type implementing
    // ApiMethodInfo, and make its instance Deref to an actual function.
    // This way the function can still be used normally. Validators for parameters will be
    // executed, serialization happens only when coming from the method's `handler`.

    let name_str = name.to_string();
    let struct_name = Ident::new(&super::util::to_camel_case(&name_str), name.span());
    let mut body = Vec::new();
    body.push(quote! {
        // This is our helper struct which Derefs to a wrapper of our original function, which
        // applies the added validators.
        #vis struct #struct_name();

        #[allow(non_upper_case_globals)]
        const #name: &#struct_name = &#struct_name();

        // Namespace some of our code into the helper type:
        impl #struct_name {
            // This is the original function, renamed to `#impl_unchecked_ident`
            #item

            // This is the handler used by our router, which extracts the parameters out of a
            // serde_json::Value, running the actual method, then serializing the output into an
            // API response.
            //
            // FIXME: For now this always returns status 200, we're going to have to figure out how
            // to use different success status values.
            //   This could be a simple optional parameter to just replace the number, or
            //   alternatively we could just recognize functions returning a http::Response and not
            //   perform the serialization/http::Response-building automatically.
            //   (Alternatively we could do exactly that with a trait so we don't have to parse the
            //   return type?)
            fn wrapped_api_handler(
                args: ::serde_json::Value,
            ) -> ::proxmox::api::ApiFuture<#body_type> {
                async fn handler(
                    mut args: ::serde_json::Value,
                ) -> ::proxmox::api::ApiOutput<#body_type> {
                    let mut empty_args = ::serde_json::map::Map::new();
                    let args = args.as_object_mut()
                        .unwrap_or(&mut empty_args);

                    #arg_extraction

                    if !args.is_empty() {
                        let mut extra = String::new();
                        for arg in args.keys() {
                            if !extra.is_empty() {
                                extra.push_str(", ");
                            }
                            extra.push_str(arg);
                        }
                        ::failure::bail!("unexpected extra parameters: {}", extra);
                    }

                    let output = #struct_name::#impl_checked_ident(#extracted_args).await?;
                    ::proxmox::api::IntoApiOutput::into_api_output(output)
                }
                Box::pin(handler(args))
            }
        }
    });

    if item.asyncness.is_some() {
        // An async function is expected to return its value, so we wrap it a bit:
        body.push(quote! {
            impl #struct_name {
                async fn #impl_checked_ident(#inputs) -> #return_type {
                    #parameter_verifiers
                    Self::#impl_unchecked_ident(#passed_args).await
                }
            }

            // Our helper type derefs to a wrapper performing input validation and returning a
            // Pin<Box<Future>>.
            // Unfortunately we cannot return the actual function since that won't work for
            // `async fn`, since an `async fn` cannot appear as a return type :(
            impl ::std::ops::Deref for #struct_name {
                type Target = fn(#inputs) -> ::std::pin::Pin<Box<
                    dyn ::std::future::Future<Output = #return_type>
                >>;

                fn deref(&self) -> &Self::Target {
                    const FUNC: fn(#inputs) -> ::std::pin::Pin<Box<dyn ::std::future::Future<
                        Output = #return_type,
                    >>> = |#inputs| {
                        Box::pin(#struct_name::#impl_checked_ident(#passed_args))
                    };
                    &FUNC
                }
            }
        });
    } else {
        // Non async fn must return an ApiFuture already!
        return_type = syn::Type::Verbatim(syn::TypeVerbatim {
            tts: definition
                .remove("returns")
                .ok_or_else(|| {
                    format_err!(
                        "non async-fn must return a Response \
                         and specify its return type via the `returns` property",
                    )
                })?
                .expect_type()?
                .into_token_stream(),
        });

        body.push(quote! {
            impl #struct_name {
                fn #impl_checked_ident(#inputs) -> ::proxmox::api::ApiFuture<#body_type> {
                    #parameter_verifiers
                    Self::#impl_unchecked_ident(#passed_args)
                }
            }

            // Our helper type derefs to a wrapper performing input validation and returning a
            // Pin<Box<Future>>.
            // Unfortunately we cannot return the actual function since that won't work for
            // `async fn`, since an `async fn` cannot appear as a return type :(
            impl ::std::ops::Deref for #struct_name {
                type Target = fn(#inputs) -> ::proxmox::api::ApiFuture<#body_type>;

                fn deref(&self) -> &Self::Target {
                    &(Self::#impl_checked_ident as Self::Target)
                }
            }
        });
    }

    body.push(quote! {
        // We now need to provide all the info required for routing, command line completion, API
        // documentation, etc.
        //
        // Note that technically we don't need the `description` member in this trait, as this is
        // mostly used at compile time for documentation!
        impl ::proxmox::api::ApiMethodInfo<#body_type> for #struct_name {
            fn description(&self) -> &'static str {
                #fn_api_description
            }

            fn parameters(&self) -> &'static [::proxmox::api::Parameter] {
                // FIXME!
                &[ #parameter_entries ]
            }

            fn return_type(&self) -> &'static ::proxmox::api::TypeInfo {
                <#return_type as ::proxmox::api::ApiType>::type_info()
            }

            fn protected(&self) -> bool {
                #fn_api_protected
            }

            fn reload_timezone(&self) -> bool {
                #fn_api_reload_timezone
            }

            fn handler(&self) -> fn(::serde_json::Value) -> ::proxmox::api::ApiFuture<#body_type> {
                #struct_name::wrapped_api_handler
            }
        }
    });

    let body = TokenStream::from_iter(body);
    //dbg!("{}", &body);
    Ok(body)
}

fn make_parameter_verifier(
    var: &Ident,
    var_str: &str,
    info: &mut HashMap<String, Expression>,
    out: &mut TokenStream,
) -> Result<(), Error> {
    match info.remove("minimum") {
        None => (),
        Some(Expression::Expr(expr)) => out.extend(quote! {
            let cmp = #expr;
            if #var < cmp {
                bail!("parameter '{}' is out of range (must be >= {})", #var_str, cmp);
            }
        }),
        Some(_) => bail!("invalid value for 'minimum'"),
    }

    match info.remove("maximum") {
        None => (),
        Some(Expression::Expr(expr)) => out.extend(quote! {
            let cmp = #expr;
            if #var > cmp {
                bail!("parameter '{}' is out of range (must be <= {})", #var_str, cmp);
            }
        }),
        Some(_) => bail!("invalid value for 'maximum'"),
    }

    Ok(())
}

fn handle_struct(
    definition: HashMap<String, Expression>,
    item: &syn::ItemStruct,
) -> Result<TokenStream, Error> {
    if item.generics.lt_token.is_some() {
        bail!("generic types are currently not supported");
    }

    let name = &item.ident;

    match item.fields {
        syn::Fields::Unit => bail!("unit types are not allowed"),
        syn::Fields::Unnamed(ref fields) => handle_struct_unnamed(definition, name, fields),
        syn::Fields::Named(ref fields) => handle_struct_named(definition, name, fields),
    }
}

fn handle_struct_unnamed(
    definition: HashMap<String, Expression>,
    name: &Ident,
    item: &syn::FieldsUnnamed,
) -> Result<TokenStream, Error> {
    let fields = &item.unnamed;
    if fields.len() != 1 {
        bail!("only 1 unnamed field is currently allowed for api types");
    }

    //let field = fields.first().unwrap().value();

    let apidef = ParameterDefinition::from_object(definition)?;

    let validator = match apidef.validate {
        Some(ident) => quote! { #ident(&self.0) },
        None => quote! { ::proxmox::api::ApiType::verify(&self.0) },
    };

    Ok(quote! {
        impl ::proxmox::api::ApiType for #name {
            fn type_info() -> &'static ::proxmox::api::TypeInfo {
                const INFO: ::proxmox::api::TypeInfo = ::proxmox::api::TypeInfo {
                    name: stringify!(#name),
                    description: "FIXME",
                    complete_fn: None, // FIXME!
                    parse_cli: Some(<#name as ::proxmox::api::cli::ParseCli>::parse_cli),
                };
                &INFO
            }

            fn verify(&self) -> Result<(), Error> {
                #validator
            }
        }
    })
}

fn handle_struct_named(
    definition: HashMap<String, Expression>,
    name: &Ident,
    item: &syn::FieldsNamed,
) -> Result<TokenStream, Error> {
    let mut verify_entries = None;
    let mut description = None;
    for (key, value) in definition {
        match key.as_str() {
            "fields" => {
                verify_entries = Some(handle_named_struct_fields(item, value.expect_object()?)?);
            }
            "description" => {
                description = Some(value.expect_lit_str()?);
            }
            other => bail!("unknown api definition field: {}", other),
        }
    }

    let description = description
        .ok_or_else(|| format_err!("missing 'description' for type {}", name.to_string()))?;

    use std::iter::FromIterator;
    let verifiers = TokenStream::from_iter(
        verify_entries.ok_or_else(|| format_err!("missing 'fields' definition for struct"))?,
    );

    Ok(quote! {
        impl ::proxmox::api::ApiType for #name {
            fn type_info() -> &'static ::proxmox::api::TypeInfo {
                const INFO: ::proxmox::api::TypeInfo = ::proxmox::api::TypeInfo {
                    name: stringify!(#name),
                    description: #description,
                    complete_fn: None, // FIXME!
                    parse_cli: Some(<#name as ::proxmox::api::cli::ParseCli>::parse_cli),
                };
                &INFO
            }

            fn verify(&self) -> Result<(), Error> {
                #verifiers
                Ok(())
            }
        }
    })
}

fn handle_named_struct_fields(
    item: &syn::FieldsNamed,
    mut field_def: HashMap<String, Expression>,
) -> Result<Vec<TokenStream>, Error> {
    let mut verify_entries = Vec::new();

    for field in item.named.iter() {
        let name = &field.ident;
        let name_str = name
            .as_ref()
            .expect("field name in struct of named fields")
            .to_string();

        let this = quote! { self.#name };

        let def = field_def
            .remove(&name_str)
            .ok_or_else(|| format_err!("missing field in definition: '{}'", name_str))?
            .expect_object()?;

        let def = ParameterDefinition::from_object(def)?;
        def.add_verifiers(&name_str, this, &mut verify_entries);
    }

    if !field_def.is_empty() {
        // once SliceConcatExt is stable we can join(",") on the fields...
        let mut missing = String::new();
        for key in field_def.keys() {
            if !missing.is_empty() {
                missing.push_str(", ");
            }
            missing.push_str(&key);
        }
        bail!(
            "the following struct fields are not handled in the api definition: {}",
            missing
        );
    }

    Ok(verify_entries)
}

//fn parse_api_definition(def: &mut ApiDefinitionBuilder, tokens: TokenStream) -> Result<(), Error> {
//    let obj = parse_object(tokens)?;
//    for (key, value) in obj {
//        match (key.as_str(), value) {
//            ("parameters", Value::Object(members)) => {
//                def.parameters(handle_parameter_list(members)?);
//            }
//            ("parameters", other) => bail!("not a parameter list: {:?}", other),
//            ("unauthenticated", value) => {
//                def.unauthenticated(value.to_bool()?);
//            }
//            (key, _) => bail!("unexpected api definition parameter: {}", key),
//        }
//    }
//    Ok(())
//}
//
//fn handle_parameter_list(obj: HashMap<String, Value>) -> Result<HashMap<String, Parameter>, Error> {
//    let mut out = HashMap::new();
//
//    for (key, value) in obj {
//        let parameter = match value {
//            Value::Description(ident, description) => {
//                make_default_parameter(&ident.to_string(), description)?
//            }
//            Value::Optional(ident, description) => {
//                let mut parameter = make_default_parameter(&ident.to_string(), description)?;
//                parameter.optional = true;
//                parameter
//            }
//            Value::Object(obj) => handle_parameter(&key, obj)?,
//            other => bail!("expected parameter type for {}, at {:?}", key, other),
//        };
//
//        if out.insert(key.clone(), parameter).is_some() {
//            bail!("duplicate parameter entry: {}", key);
//        }
//    }
//
//    Ok(out)
//}
//
//fn make_default_parameter(ident: &str, description: String) -> Result<Parameter, Error> {
//    let mut parameter = Parameter::default();
//    parameter.description = description;
//    parameter.parameter_type = match ident {
//        "bool" => ParameterType::Bool,
//        "string" => ParameterType::String(StringParameter::default()),
//        "float" => ParameterType::Float(FloatParameter::default()),
//        "object" => {
//            let mut obj = ObjectParameter::default();
//            obj.allow_unknown_keys = true;
//            ParameterType::Object(obj)
//        }
//        other => bail!("invalid parameter type name: {}", other),
//    };
//    Ok(parameter)
//}
//
//fn handle_parameter(key: &str, mut obj: HashMap<String, Value>) -> Result<Parameter, Error> {
//    let mut builder = ParameterBuilder::default();
//
//    builder.name(key.to_string());
//
//    if let Some(optional) = obj.remove("optional") {
//        builder.optional(optional.to_bool()?);
//    } else {
//        builder.optional(false);
//    }
//
//    builder.description(
//        obj.remove("description")
//            .ok_or_else(|| {
//                format_err!("`description` field is not optional in parameter definition")
//            })?
//            .to_string()?,
//    );
//
//    let type_name = obj
//        .remove("type")
//        .ok_or_else(|| format_err!("missing type name in parameter {}", key))?;
//
//    let type_name = match type_name {
//        Value::Ident(ident) => ident.to_string(),
//        other => bail!("bad type name for parameter {}: {:?}", key, other),
//    };
//
//    builder.parameter_type(match type_name.as_str() {
//        "integer" => handle_integer_type(&mut obj)?,
//        "float" => handle_float_type(&mut obj)?,
//        "string" => handle_string_type(&mut obj)?,
//        _ => bail!("unknown type name: {}", type_name),
//    });
//
//    if !obj.is_empty() {
//        bail!(
//            "unknown keys for type {}: {}",
//            type_name,
//            obj.keys().fold(String::new(), |acc, key| {
//                if acc.is_empty() {
//                    key.to_string()
//                } else {
//                    format!("{}, {}", acc, key)
//                }
//            })
//        )
//    }
//
//    builder.build().map_err(|e| format_err!("{}", e))
//}
//
//fn handle_string_type(obj: &mut HashMap<String, Value>) -> Result<ParameterType, Error> {
//    let mut param = StringParameter::default();
//
//    if let Some(value) = obj.remove("minimum_length") {
//        param.minimum_length = Some(value.to_unsigned()?);
//    }
//
//    if let Some(value) = obj.remove("maximum_length") {
//        param.maximum_length = Some(value.to_unsigned()?);
//    }
//
//    Ok(ParameterType::String(param))
//}
//
//fn handle_integer_type(obj: &mut HashMap<String, Value>) -> Result<ParameterType, Error> {
//    let mut param = IntegerParameter::default();
//
//    if let Some(value) = obj.remove("minimum") {
//        param.minimum = Some(value.to_integer()?);
//    }
//
//    if let Some(value) = obj.remove("maximum") {
//        param.maximum = Some(value.to_integer()?);
//    }
//
//    Ok(ParameterType::Integer(param))
//}
//
//fn handle_float_type(obj: &mut HashMap<String, Value>) -> Result<ParameterType, Error> {
//    let mut param = FloatParameter::default();
//
//    if let Some(value) = obj.remove("minimum") {
//        param.minimum = Some(value.to_float()?);
//    }
//
//    if let Some(value) = obj.remove("maximum") {
//        param.maximum = Some(value.to_float()?);
//    }
//
//    Ok(ParameterType::Float(param))
//}
