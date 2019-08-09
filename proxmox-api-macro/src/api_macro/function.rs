//! Module for function handling.

use proc_macro2::{Ident, Span, TokenStream};

use failure::{bail, format_err, Error};
use quote::{quote, ToTokens};
use syn::{spanned::Spanned, Expr, Token};

use crate::parsing::{Expression, Object};
use crate::util;

pub fn handle_function(
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
    let struct_name = Ident::new(&util::to_camel_case(&name_str), name.span());
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
