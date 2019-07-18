use proc_macro2::{Delimiter, Ident, Span, TokenStream, TokenTree};

use failure::{bail, format_err, Error};
use quote::{quote, quote_spanned, ToTokens};
use syn::{spanned::Spanned, Expr, Token};

use crate::api_def::{CommonTypeDefinition, ParameterDefinition};
use crate::parsing::*;
use crate::util;

pub fn api_macro(attr: TokenStream, item: TokenStream) -> Result<TokenStream, Error> {
    let definition = attr
        .into_iter()
        .next()
        .expect("expected api definition in braces");

    let definition = match definition {
        TokenTree::Group(ref group) if group.delimiter() == Delimiter::Brace => group.stream(),
        _ => c_bail!(definition => "expected api definition in braces"),
    };

    let def_span = definition.span();
    let definition = parse_object(definition)?;

    // Now parse the item, based on which we decide whether this is an API method which needs a
    // wrapper, or an API type which needs an ApiType implementation!
    let mut item: syn::Item = syn::parse2(item).unwrap();

    match item {
        syn::Item::Struct(ref itemstruct) => {
            let extra = handle_struct(definition, itemstruct)?;
            let mut output = item.into_token_stream();
            output.extend(extra);
            Ok(output)
        }
        syn::Item::Fn(func) => handle_function(def_span, definition, func),
        syn::Item::Enum(ref mut itemenum) => {
            let extra = handle_enum(definition, itemenum)?;
            let mut output = item.into_token_stream();
            output.extend(extra);
            Ok(output)
        }
        _ => c_bail!(item => "api macro currently only applies to structs and functions"),
    }
}

fn handle_function(
    def_span: Span,
    mut definition: Object,
    mut item: syn::ItemFn,
) -> Result<TokenStream, Error> {
    if item.decl.generics.lt_token.is_some() {
        c_bail!(
            item.decl.generics.span(),
            "cannot use generic functions for api macros currently",
        );
        // Not until we stabilize our generated representation!
    }

    // We cannot use #{foo.bar} in quote!, we can only use #foo, so these must all be local
    // variables. (I'd prefer a struct and using `#{func.description}`, `#{func.protected}` etc.
    // but that's not supported.

    let fn_api_description = definition
        .remove("description")
        .ok_or_else(|| c_format_err!(def_span, "missing 'description' in method definition"))?
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
        .unwrap_or_else(|| Object::new(Span::call_site()));
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
            list.push_str(param.as_str());
        }
        bail!(
            "api definition contains parameters not found in function declaration: {}",
            list
        );
    }

    use std::iter::FromIterator;
    let arg_extraction = TokenStream::from_iter(arg_extraction.into_iter());

    // The router expects an ApiMethod, or more accurately, an object implementing ApiHandler.
    // This is because we need access to a bunch of additional attributes of the functions both at
    // runtime and when doing command line parsing/completion/help output.
    //
    // When manually implementing methods, we usually just write them out as an `ApiMethod` which
    // is a type requiring all the info made available by the ApiHandler trait as members.
    //
    // While we could just generate a `const ApiMethod` for our functions, we would like them to
    // also be usable as functions simply because the syntax we use to create them makes them
    // *look* like functions, so it would be nice if they also *behaved* like real functions.
    //
    // Therefore all the fields of an ApiMethod are accessed via methods from the ApiHandler trait
    // and we perform the same trick lazy_static does: Create a new type implementing ApiHandler,
    // and make its instance Deref to an actual function.
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
        impl ::proxmox::api::ApiMethodInfo for #struct_name {
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
        }

        impl ::proxmox::api::ApiHandler for #struct_name {
            type Body = #body_type;

            fn call(&self, params: ::serde_json::Value) -> ::proxmox::api::ApiFuture<#body_type> {
                #struct_name::wrapped_api_handler(params)
            }

            fn method_info(&self) -> &(dyn ::proxmox::api::ApiMethodInfo + Send + Sync) {
                self as _
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
    info: &mut Object,
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

fn handle_struct(definition: Object, item: &syn::ItemStruct) -> Result<TokenStream, Error> {
    if item.generics.lt_token.is_some() {
        c_bail!(
            item.generics.span(),
            "generic types are currently not supported"
        );
    }

    let name = &item.ident;

    match item.fields {
        syn::Fields::Unit => bail!("unit types are not allowed"),
        syn::Fields::Unnamed(ref fields) => handle_struct_unnamed(definition, name, fields),
        syn::Fields::Named(ref fields) => handle_struct_named(definition, name, fields),
    }
}

fn handle_struct_unnamed(
    mut definition: Object,
    name: &Ident,
    item: &syn::FieldsUnnamed,
) -> Result<TokenStream, Error> {
    let fields = &item.unnamed;
    if fields.len() != 1 {
        bail!("only 1 unnamed field is currently allowed for api types");
    }

    //let field = fields.first().unwrap().value();

    let common = CommonTypeDefinition::from_object(&mut definition)?;
    let apidef = ParameterDefinition::from_object(definition)?;

    let validator = match apidef.validate {
        Some(ident) => quote! { #ident(&self.0) },
        None => quote! { ::proxmox::api::ApiType::verify(&self.0) },
    };

    let description = common.description;
    let parse_cli = common.cli.quote(&name);
    Ok(quote! {
        impl ::proxmox::api::ApiType for #name {
            fn type_info() -> &'static ::proxmox::api::TypeInfo {
                use ::proxmox::api::cli::ParseCli;
                use ::proxmox::api::cli::ParseCliFromStr;
                const INFO: ::proxmox::api::TypeInfo = ::proxmox::api::TypeInfo {
                    name: stringify!(#name),
                    description: #description,
                    complete_fn: None, // FIXME!
                    parse_cli: #parse_cli,
                };
                &INFO
            }

            fn verify(&self) -> ::std::result::Result<(), ::failure::Error> {
                #validator
            }
        }
    })
}

fn handle_struct_named(
    mut definition: Object,
    name: &Ident,
    item: &syn::FieldsNamed,
) -> Result<TokenStream, Error> {
    let mut verify_entries = None;
    let common = CommonTypeDefinition::from_object(&mut definition)?;
    for (key, value) in definition {
        match key.as_str() {
            "fields" => {
                verify_entries = Some(handle_named_struct_fields(item, value.expect_object()?)?);
            }
            other => bail!("unknown api definition field: {}", other),
        }
    }

    use std::iter::FromIterator;
    let verifiers = TokenStream::from_iter(
        verify_entries.ok_or_else(|| format_err!("missing 'fields' definition for struct"))?,
    );

    let description = common.description;
    let parse_cli = common.cli.quote(&name);
    Ok(quote! {
        impl ::proxmox::api::ApiType for #name {
            fn type_info() -> &'static ::proxmox::api::TypeInfo {
                const INFO: ::proxmox::api::TypeInfo = ::proxmox::api::TypeInfo {
                    name: stringify!(#name),
                    description: #description,
                    complete_fn: None, // FIXME!
                    parse_cli: #parse_cli,
                };
                &INFO
            }

            fn verify(&self) -> ::std::result::Result<(), ::failure::Error> {
                #verifiers
                Ok(())
            }
        }
    })
}

fn handle_named_struct_fields(
    item: &syn::FieldsNamed,
    mut field_def: Object,
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
            .ok_or_else(|| {
                c_format_err!(name.span(), "missing field in definition: '{}'", name_str)
            })?
            .expect_object()?;

        let def = ParameterDefinition::from_object(def)?;
        def.add_verifiers(&name_str, this, &mut verify_entries);
    }

    if !field_def.is_empty() {
        // once SliceConcatExt is stable we can join(",") on the fields...
        let mut span = None;
        let mut missing = String::new();
        for key in field_def.keys() {
            if !missing.is_empty() {
                missing.push_str(", ");
            }
            if span.is_none() {
                span = Some(key.span());
            }
            missing.push_str(key.as_str());
        }
        c_bail!(
            span.unwrap(),
            "the following struct fields are not handled in the api definition: {}",
            missing
        );
    }

    Ok(verify_entries)
}

/// Enums are string types. Note that we usually use lower case enum values, but rust wants
/// CamelCase, so unless otherwise requested by the user (todo!), we convert CamelCase to
/// underscore_case automatically.
///
/// For enums we automatically implement `ToString`, `FromStr`, and derive `Serialize` and
/// `Deserialize` via `serde_plain`.
fn handle_enum(mut definition: Object, item: &mut syn::ItemEnum) -> Result<TokenStream, Error> {
    if item.generics.lt_token.is_some() {
        c_bail!(
            item.generics.span(),
            "generic types are currently not supported"
        );
    }

    let enum_ident = &item.ident;
    let enum_name = enum_ident.to_string();
    let expected = format!("valid {}", enum_ident);

    let mut display_entries = TokenStream::new();
    let mut from_str_entries = TokenStream::new();

    for variant in item.variants.iter_mut() {
        if variant.fields != syn::Fields::Unit {
            c_bail!(variant.span(), "#[api] enums cannot have fields");
        }

        let variant_ident = &variant.ident;
        let span = variant_ident.span();
        let underscore_name = util::to_underscore_case(&variant_ident.to_string());

        display_entries.extend(quote_spanned! {
            span => #enum_ident::#variant_ident => write!(f, #underscore_name),
        });

        from_str_entries.extend(quote_spanned! {
            span => #underscore_name => Ok(#enum_ident::#variant_ident),
        });
    }

    let common = CommonTypeDefinition::from_object(&mut definition)?;
    let apidef = ParameterDefinition::from_object(definition)?;

    if let Some(validate) = apidef.validate {
        c_bail!(validate => "validators are not allowed on enum types");
    }

    let description = common.description;
    let parse_cli = common.cli.quote(&enum_ident);
    Ok(quote_spanned! { item.span() =>
        impl ::std::fmt::Display for #enum_ident {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                match self {
                    #display_entries
                }
            }
        }

        impl ::std::str::FromStr for #enum_ident {
            type Err = ::failure::Error;

            fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
                match s {
                    #from_str_entries
                    _ => ::failure::bail!("expected {}", #expected),
                }
            }
        }

        ::serde_plain::derive_deserialize_from_str!(#enum_ident, #expected);
        ::serde_plain::derive_serialize_from_display!(#enum_ident);

        ::proxmox::api::derive_parse_cli_from_str!(#enum_ident);

        impl ::proxmox::api::ApiType for #enum_ident {
            fn type_info() -> &'static ::proxmox::api::TypeInfo {
                const INFO: ::proxmox::api::TypeInfo = ::proxmox::api::TypeInfo {
                    name: #enum_name,
                    description: #description,
                    complete_fn: None, // FIXME!
                    parse_cli: #parse_cli,
                };
                &INFO
            }

            fn verify(&self) -> ::std::result::Result<(), ::failure::Error> {
                Ok(())
            }
        }
    })
}
