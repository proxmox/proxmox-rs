use std::mem;

use proc_macro2::{Delimiter, Ident, Span, TokenStream, TokenTree};

use failure::{bail, Error};
use quote::{quote, quote_spanned, ToTokens};
use syn::{spanned::Spanned};

use crate::api_def::{CommonTypeDefinition, ParameterDefinition};
use crate::parsing::*;
use crate::util;

mod function;

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
        syn::Item::Struct(mut itemstruct) => {
            let extra = handle_struct(definition, &mut itemstruct)?;
            let mut output = itemstruct.into_token_stream();
            output.extend(extra);
            Ok(output)
        }
        syn::Item::Fn(func) => function::handle_function(def_span, definition, func),
        syn::Item::Enum(ref mut itemenum) => {
            let extra = handle_enum(definition, itemenum)?;
            let mut output = item.into_token_stream();
            output.extend(extra);
            Ok(output)
        }
        _ => c_bail!(item => "api macro currently only applies to structs and functions"),
    }
}

fn handle_struct(definition: Object, item: &mut syn::ItemStruct) -> Result<TokenStream, Error> {
    if item.generics.lt_token.is_some() {
        c_bail!(
            item.generics.span(),
            "generic types are currently not supported"
        );
    }

    let name = &item.ident;

    match item.fields {
        syn::Fields::Unit => c_bail!(item.span(), "unit types are not allowed"),
        syn::Fields::Unnamed(ref fields) if fields.unnamed.len() == 1 => {
            handle_newtype(definition, name, fields, &mut item.attrs)
        }
        syn::Fields::Unnamed(ref fields) => handle_struct_unnamed(definition, name, fields),
        syn::Fields::Named(ref fields) => handle_struct_named(definition, name, fields),
    }
}

struct StructField<'i, 't> {
    def: ParameterDefinition,
    ident: Option<&'i Ident>,
    access: syn::Member,
    mem_id: isize,
    string: String,
    strlit: syn::LitStr,
    ty: &'t syn::Type,
}

fn handle_newtype(
    mut definition: Object,
    type_ident: &Ident,
    item: &syn::FieldsUnnamed,
    attrs: &mut Vec<syn::Attribute>,
) -> Result<TokenStream, Error> {
    let type_s = type_ident.to_string();
    let type_span = type_ident.span();
    let type_str = syn::LitStr::new(&type_s, type_span);

    let fields = &item.unnamed;
    let field_punct = fields.first().unwrap();
    let field = field_punct.value();

    let common = CommonTypeDefinition::from_object(&mut definition)?;

    let serialize_as_string = definition
        .remove("serialize_as_string")
        .map(|e| e.expect_lit_bool_direct())
        .transpose()?
        .unwrap_or(false);

    let apidef = ParameterDefinition::from_object(definition)?;

    let impl_verify = struct_fields_impl_verify(
        item.span(),
        &[StructField {
            def: apidef,
            ident: None,
            access: syn::Member::Unnamed(syn::Index {
                index: 0,
                span: type_ident.span(),
            }),
            mem_id: 0,
            string: "0".to_string(),
            strlit: syn::LitStr::new("0", type_ident.span()),
            ty: &field.ty,
        }],
    )?;

    let (impl_serialize, impl_deserialize) = if serialize_as_string {
        let expected = format!("valid {}", type_ident);
        (
            quote_spanned! { item.span() =>
                ::serde_plain::derive_serialize_from_display!(#type_ident);
            },
            quote_spanned! { item.span() =>
                ::serde_plain::derive_deserialize_from_str!(#type_ident, #expected);
            },
        )
    } else {
        (
            newtype_derive_serialize(item.span(), type_ident),
            newtype_derive_deserialize(item.span(), type_ident),
        )
    };

    let derive_impls = newtype_filter_derive_attrs(type_ident, &field.ty, attrs)?;

    let description = common.description;
    let parse_cli = common.cli.quote(&type_ident);
    Ok(quote! {
        #impl_serialize

        #impl_deserialize

        #derive_impls

        impl ::proxmox::api::ApiType for #type_ident {
            fn type_info() -> &'static ::proxmox::api::TypeInfo {
                use ::proxmox::api::cli::ParseCli;
                use ::proxmox::api::cli::ParseCliFromStr;
                const INFO: ::proxmox::api::TypeInfo = ::proxmox::api::TypeInfo {
                    name: #type_str,
                    description: #description,
                    complete_fn: None, // FIXME!
                    parse_cli: #parse_cli,
                };
                &INFO
            }

            #impl_verify
        }
    })
}

fn newtype_derive_serialize(span: Span, type_ident: &Ident) -> TokenStream {
    quote_spanned! { span =>
        impl ::serde::ser::Serialize for #type_ident {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: ::serde::ser::Serializer,
            {
                ::serde::ser::Serialize::serialize::<S>(&self.0, serializer)
            }
        }
    }
}

fn newtype_derive_deserialize(span: Span, type_ident: &Ident) -> TokenStream {
    quote_spanned! { span =>
        impl<'de> ::serde::de::Deserialize<'de> for #type_ident {
            fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
            where
                D: ::serde::de::Deserializer<'de>,
            {
                Ok(Self(::serde::de::Deserialize::<'de>::deserialize::<D>(deserializer)?))
            }
        }
    }
}

fn newtype_filter_derive_attrs(
    type_ident: &Ident,
    inner_type: &syn::Type,
    attrs: &mut Vec<syn::Attribute>,
) -> Result<TokenStream, Error> {
    let mut code = TokenStream::new();
    let mut had_from_str = false;

    let cap = attrs.len();
    for mut attr in mem::replace(attrs, Vec::with_capacity(cap)) {
        if !attr.path.is_ident("derive") {
            attrs.push(attr);
            continue;
        }

        let mut content: syn::Expr = syn::parse2(attr.tts)?;
        if let syn::Expr::Tuple(ref mut exprtuple) = content {
            for ty in mem::replace(&mut exprtuple.elems, syn::punctuated::Punctuated::new()) {
                if let syn::Expr::Path(ref exprpath) = ty {
                    if exprpath.path.is_ident("FromStr") {
                        if !had_from_str {
                            code.extend(newtype_derive_from_str(
                                exprpath.path.span(),
                                type_ident,
                                inner_type,
                            ));
                        }
                        had_from_str = true;
                        continue;
                    }
                }
                exprtuple.elems.push(ty);
            }
        }
        attr.tts = quote! { #content };
        attrs.push(attr);
    }

    Ok(code)
}

fn newtype_derive_from_str(span: Span, type_ident: &Ident, inner_type: &syn::Type) -> TokenStream {
    quote_spanned! { span =>
        impl ::std::str::FromStr for #type_ident {
            type Err = <#inner_type as ::std::str::FromStr>::Err;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(::std::str::FromStr::from_str(s)?))
            }
        }
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
    type_ident: &Ident,
    item: &syn::FieldsNamed,
) -> Result<TokenStream, Error> {
    let common = CommonTypeDefinition::from_object(&mut definition)?;
    let mut field_def = definition
        .remove("fields")
        .ok_or_else(|| c_format_err!(definition.span(), "missing 'fields' entry"))?
        .expect_object()?;

    let derive_default = definition
        .remove("derive_default")
        .map(|e| e.expect_lit_bool_direct())
        .transpose()?
        .unwrap_or(false);

    if derive_default {
        // We currently fill the actual `default` values from the schema into Option<Foo>, but
        // really Option<Foo> should default to None even when there's a Default as our accessors
        // will fill in the default at use-time...
        bail!("derive_default is not finished");
    }

    let serialize_as_string = definition
        .remove("serialize_as_string")
        .map(|e| e.expect_lit_bool_direct())
        .transpose()?
        .unwrap_or(false);

    let type_s = type_ident.to_string();
    let type_span = type_ident.span();
    let type_str = syn::LitStr::new(&type_s, type_span);

    let mut mem_id: isize = 0;
    let mut fields = Vec::new();
    for field in item.named.iter() {
        mem_id += 1;
        let field_ident = field
            .ident
            .as_ref()
            .ok_or_else(|| c_format_err!(field => "missing field name"))?;
        let field_string = field_ident.to_string();

        let field_strlit = syn::LitStr::new(&field_string, field_ident.span());

        let def = field_def.remove(&field_string).ok_or_else(
            || c_format_err!(field => "missing api description entry for field {}", field_string),
        )?;
        let def = ParameterDefinition::from_expression(def)?;
        fields.push(StructField {
            def,
            ident: Some(field_ident),
            access: syn::Member::Named(field_ident.clone()),
            mem_id,
            string: field_string,
            strlit: field_strlit,
            ty: &field.ty,
        });
    }

    let impl_verify = struct_fields_impl_verify(item.span(), &fields)?;
    let (impl_serialize, impl_deserialize) = if serialize_as_string {
        let expected = format!("valid {}", type_ident);
        (
            quote_spanned! { item.span() =>
                ::serde_plain::derive_serialize_from_display!(#type_ident);
            },
            quote_spanned! { item.span() =>
                ::serde_plain::derive_deserialize_from_str!(#type_ident, #expected);
            },
        )
    } else {
        (
            named_struct_derive_serialize(item.span(), type_ident, &type_str, &fields)?,
            named_struct_derive_deserialize(item.span(), type_ident, &type_str, &fields)?,
        )
    };

    let accessors = named_struct_impl_accessors(item.span(), type_ident, &fields)?;

    let impl_default = if derive_default {
        named_struct_impl_default(item.span(), type_ident, &fields)?
    } else {
        TokenStream::new()
    };

    let description = common.description;
    let parse_cli = common.cli.quote(&type_ident);
    Ok(quote_spanned! { item.span() =>
        #impl_serialize

        #impl_deserialize

        #impl_default

        #accessors

        impl ::proxmox::api::ApiType for #type_ident {
            fn type_info() -> &'static ::proxmox::api::TypeInfo {
                const INFO: ::proxmox::api::TypeInfo = ::proxmox::api::TypeInfo {
                    name: #type_str,
                    description: #description,
                    complete_fn: None, // FIXME!
                    parse_cli: #parse_cli,
                };
                &INFO
            }

            #impl_verify
        }
    })
}

fn struct_fields_impl_verify(span: Span, fields: &[StructField]) -> Result<TokenStream, Error> {
    let mut body = TokenStream::new();
    for field in fields {
        let field_access = &field.access;
        let field_str = &field.strlit;

        // first of all, recurse into the contained types:
        body.extend(quote_spanned! { field_access.span() =>
            ::proxmox::api::ApiType::verify(&self.#field_access)?;
        });

        // then go through all the additional verifiers:

        if let Some(ref value) = field.def.minimum {
            body.extend(quote_spanned! { value.span() =>
                let value = #value;
                if !::proxmox::api::verify::TestMinMax::test_minimum(&self.#field_access, &value) {
                    error_list.push(
                        format!("field {} out of range, must be >= {}", #field_str, value)
                    );
                }
            });
        }

        if let Some(ref value) = field.def.maximum {
            body.extend(quote_spanned! { value.span() =>
                let value = #value;
                if !::proxmox::api::verify::TestMinMax::test_maximum(&self.#field_access, &value) {
                    error_list.push(
                        format!("field {} out of range, must be <= {}", #field_str, value)
                    );
                }
            });
        }

        if let Some(ref value) = field.def.minimum_length {
            body.extend(quote_spanned! { value.span() =>
                let value = #value;
                if !::proxmox::api::verify::TestMinMaxLen::test_minimum_length(
                    &self.#field_access,
                    value,
                ) {
                    error_list.push(
                        format!("field {} too short, must be >= {} characters", #field_str, value)
                    );
                }
            });
        }

        if let Some(ref value) = field.def.maximum_length {
            body.extend(quote_spanned! { value.span() =>
                let value = #value;
                if !::proxmox::api::verify::TestMinMaxLen::test_maximum_length(
                    &self.#field_access,
                    value,
                ) {
                    error_list.push(
                        format!("field {} too long, must be <= {} characters", #field_str, value)
                    );
                }
            });
        }

        if let Some(ref value) = field.def.format {
            body.extend(quote_spanned! { value.span() =>
                if !#value::verify(&self.#field_access) {
                    error_list.push(
                        format!("field {} does not match format {}", #field_str, #value::NAME)
                    );
                }
            });
        }

        if let Some(ref value) = field.def.pattern {
            match value {
                syn::Expr::Lit(regex) => body.extend(quote_spanned! { value.span() =>
                    {
                        ::lazy_static::lazy_static! {
                            static ref RE: ::regex::Regex = ::regex::Regex::new(#regex).unwrap();
                        }
                        if !RE.is_match(&self.#field_access) {
                            error_list.push(format!(
                                "field {} does not match the allowed pattern: {}",
                                #field_str,
                                #regex,
                            ));
                        }
                    }
                }),
                regex => body.extend(quote_spanned! { value.span() =>
                    if !#regex.is_match(&self.#field_access) {
                        error_list.push(
                            format!("field {} does not match the allowed pattern", #field_str)
                        );
                    }
                }),
            }
        }

        if let Some(ref value) = field.def.validate {
            body.extend(quote_spanned! { value.span() =>
                if let Err(err) = #value(&self.#field_access) {
                    error_list.push(err.to_string());
                }
            });
        }
    }

    if !body.is_empty() {
        body = quote_spanned! { span =>
            #[allow(unused_mut)]
            let mut error_list: Vec<String> = Vec::new();
            #body
            if !error_list.is_empty() {
                let mut error_string = String::new();
                for e in error_list.iter() {
                    if !error_string.is_empty() {
                        error_string.push_str("\n");
                    }
                    error_string.push_str(&e);
                }
                return Err(::failure::format_err!("{}", error_string));
            }
        };
    }

    Ok(quote_spanned! { span =>
        fn verify(&self) -> ::std::result::Result<(), ::failure::Error> {
            #body

            Ok(())
        }
    })
}

fn wrap_serialize_with(
    span: Span,
    name: &Ident,
    ty: &syn::Type,
    with: &syn::Path,
) -> (TokenStream, Ident) {
    let helper_name = Ident::new(
        &format!(
            "SerializeWith{}",
            crate::util::to_camel_case(&name.to_string())
        ),
        name.span(),
    );

    (
        quote_spanned! { span =>
            struct #helper_name<'a>(&'a #ty);

            impl<'a> ::serde::ser::Serialize for #helper_name<'a> {
                fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
                where
                    S: ::serde::ser::Serializer,
                {
                    #with(self.0, serializer)
                }
            }
        },
        helper_name,
    )
}

fn named_struct_derive_serialize(
    span: Span,
    type_ident: &Ident,
    type_str: &syn::LitStr,
    fields: &[StructField],
) -> Result<TokenStream, Error> {
    let field_count = fields.len();

    let mut entries = TokenStream::new();
    for field in fields {
        let field_ident = field.ident.unwrap();
        let field_span = field_ident.span();
        let field_str = &field.strlit;
        match field.def.serialize_with.as_ref() {
            Some(path) => {
                let (serializer, serializer_name) =
                    wrap_serialize_with(field_span, field_ident, &field.ty, path);

                entries.extend(quote_spanned! { field_span =>
                    if !::proxmox::api::ApiType::should_skip_serialization(&self.#field_ident) {
                        #serializer

                        state.serialize_field(#field_str, &#serializer_name(&self.#field_ident))?;
                    }
                });
            }
            None => {
                entries.extend(quote_spanned! { field_span =>
                    if !::proxmox::api::ApiType::should_skip_serialization(&self.#field_ident) {
                        state.serialize_field(#field_str, &self.#field_ident)?;
                    }
                });
            }
        }
    }

    Ok(quote_spanned! { span =>
        impl ::serde::ser::Serialize for #type_ident {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: ::serde::ser::Serializer,
            {
                use ::serde::ser::SerializeStruct;
                let mut state = serializer.serialize_struct(#type_str, #field_count)?;
                #entries
                state.end()
            }
        }
    })
}

fn wrap_deserialize_with(
    span: Span,
    name: &Ident,
    ty: &syn::Type,
    with: &syn::Path,
) -> (TokenStream, Ident) {
    let helper_name = Ident::new(
        &format!(
            "DeserializeWith{}",
            crate::util::to_camel_case(&name.to_string())
        ),
        name.span(),
    );

    (
        quote_spanned! { span =>
            struct #helper_name<'de> {
                value: #ty,
                _lifetime: ::std::marker::PhantomData<&'de ()>,
            }

            impl<'de> ::serde::de::Deserialize<'de> for #helper_name<'de> {
                fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
                where
                    D: ::serde::de::Deserializer<'de>,
                {
                    Ok(Self {
                        value: #with(deserializer)?,
                        _lifetime: ::std::marker::PhantomData,
                    })
                }
            }
        },
        helper_name,
    )
}

fn named_struct_derive_deserialize(
    span: Span,
    type_ident: &Ident,
    type_str: &syn::LitStr,
    fields: &[StructField],
) -> Result<TokenStream, Error> {
    let type_s = type_ident.to_string();
    let struct_type_str = syn::LitStr::new(&format!("struct {}", type_s), type_ident.span());
    let struct_type_field_str =
        syn::LitStr::new(&format!("struct {} field name", type_s), type_ident.span());
    let visitor_ident = Ident::new(&format!("{}Visitor", type_s), type_ident.span());

    let mut field_ident_list = TokenStream::new(); // ` member1, member2, `
    let mut field_name_matches = TokenStream::new(); // ` "member0" => 0, "member1" => 1, `
    let mut field_name_str_list = TokenStream::new(); // ` "member1", "member2", `
    let mut field_option_check_or_default_list = TokenStream::new();
    let mut field_option_init_list = TokenStream::new();
    let mut field_value_matches = TokenStream::new();
    for field in fields {
        let field_ident = field.ident.unwrap();
        let field_span = field_ident.span();
        let field_str = &field.strlit;
        let mem_id = field.mem_id;

        field_ident_list.extend(quote_spanned! { field_span => #field_ident, });

        field_name_matches.extend(quote_spanned! { field_span =>
            #field_str => Field(#mem_id),
        });

        field_name_str_list.extend(quote_spanned! { field_span => #field_str, });

        field_option_check_or_default_list.extend(quote_spanned! { field_span =>
            let #field_ident = ::proxmox::api::ApiType::deserialization_check(
                #field_ident,
                || ::serde::de::Error::missing_field(#field_str),
            )?;
        });

        match field.def.deserialize_with.as_ref() {
            Some(path) => {
                let (deserializer, deserializer_name) =
                    wrap_deserialize_with(field_span, field_ident, &field.ty, path);

                field_option_init_list.extend(quote_spanned! { field_span =>
                    #deserializer

                    let mut #field_ident = None;
                });

                field_value_matches.extend(quote_spanned! { field_span =>
                    Field(#mem_id) => {
                        if #field_ident.is_some() {
                            return Err(::serde::de::Error::duplicate_field(#field_str));
                        }
                        let tmp: #deserializer_name = _api_macro_map_.next_value()?;
                        #field_ident = Some(tmp.value);
                    }
                });
            }
            None => {
                field_option_init_list.extend(quote_spanned! { field_span =>
                    let mut #field_ident = None;
                });

                field_value_matches.extend(quote_spanned! { field_span =>
                    Field(#mem_id) => {
                        if #field_ident.is_some() {
                            return Err(::serde::de::Error::duplicate_field(#field_str));
                        }
                        #field_ident = Some(_api_macro_map_.next_value()?);
                    }
                });
            }
        }
    }

    Ok(quote_spanned! { span =>
        impl<'de> ::serde::de::Deserialize<'de> for #type_ident {
            fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
            where
                D: ::serde::de::Deserializer<'de>,
            {
                #[repr(transparent)]
                struct Field(isize);

                impl<'de> ::serde::de::Deserialize<'de> for Field {
                    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
                    where
                        D: ::serde::de::Deserializer<'de>,
                    {
                        struct FieldVisitor;

                        impl<'de> ::serde::de::Visitor<'de> for FieldVisitor {
                            type Value = Field;

                            fn expecting(
                                &self,
                                formatter: &mut ::std::fmt::Formatter,
                            ) -> ::std::fmt::Result {
                                formatter.write_str(#struct_type_field_str)
                            }

                            fn visit_str<E>(self, value: &str) -> ::std::result::Result<Field, E>
                            where
                                E: ::serde::de::Error,
                            {
                                Ok(match value {
                                    #field_name_matches
                                    _ => {
                                        return Err(
                                            ::serde::de::Error::unknown_field(value, FIELDS)
                                        );
                                    }
                                })
                            }
                        }

                        deserializer.deserialize_identifier(FieldVisitor)
                    }
                }

                struct #visitor_ident;

                impl<'de> ::serde::de::Visitor<'de> for #visitor_ident {
                    type Value = #type_ident;

                    fn expecting(
                        &self,
                        formatter: &mut ::std::fmt::Formatter,
                    ) -> ::std::fmt::Result {
                        formatter.write_str(#struct_type_str)
                    }

                    fn visit_map<V>(
                        self,
                        mut _api_macro_map_: V,
                    ) -> ::std::result::Result<#type_ident, V::Error>
                    where
                        V: ::serde::de::MapAccess<'de>,
                    {
                        #field_option_init_list
                        while let Some(_api_macro_key_) = _api_macro_map_.next_key()? {
                            match _api_macro_key_ {
                                #field_value_matches
                                _ => unreachable!(),
                            }
                        }
                        #field_option_check_or_default_list
                        Ok(#type_ident {
                            #field_ident_list
                        })
                    }
                }

                const FIELDS: &'static [&'static str] = &[ #field_name_str_list ];
                deserializer.deserialize_struct(#type_str, FIELDS, #visitor_ident)
            }
        }
    })
}

fn named_struct_impl_accessors(
    span: Span,
    type_ident: &Ident,
    fields: &[StructField],
) -> Result<TokenStream, Error> {
    let mut accessor_methods = TokenStream::new();

    for field in fields {
        if let Some(ref default) = field.def.default {
            let field_ident = field.ident;
            let field_ty = &field.ty;
            let set_field_ident = Ident::new(&format!("set_{}", field.string), field_ident.span());

            accessor_methods.extend(quote_spanned! { default.span() =>
                pub fn #field_ident(
                    &self,
                ) -> &<#field_ty as ::proxmox::api::meta::OrDefault>::Output {
                    const DEF: <#field_ty as ::proxmox::api::meta::OrDefault>::Output = #default;
                    ::proxmox::api::meta::OrDefault::or_default(&self.#field_ident, &DEF)
                }

                pub fn #set_field_ident(
                    &mut self,
                    value: <#field_ty as ::proxmox::api::meta::OrDefault>::Output,
                ) {
                    ::proxmox::api::meta::OrDefault::set(&mut self.#field_ident, value)
                }
            });
        }
    }

    Ok(quote_spanned! { span =>
        impl #type_ident {
            #accessor_methods
        }
    })
}

fn named_struct_impl_default(
    span: Span,
    type_ident: &Ident,
    fields: &[StructField],
) -> Result<TokenStream, Error> {
    let mut entries = TokenStream::new();
    for field in fields {
        let field_ident = field.ident;
        if let Some(ref default) = field.def.default {
            entries.extend(quote_spanned! { field_ident.span() =>
                #field_ident: #default.into(),
            });
        } else {
            entries.extend(quote_spanned! { field_ident.span() =>
                #field_ident: Default::default(),
            });
        }
    }
    Ok(quote_spanned! { span =>
        impl ::std::default::Default for #type_ident {
            fn default() -> Self {
                Self {
                    #entries
                }
            }
        }
    })
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
        let mut underscore_name = syn::LitStr::new(&underscore_name, variant_ident.span());

        let cap = variant.attrs.len();
        for attr in mem::replace(&mut variant.attrs, Vec::with_capacity(cap)) {
            if attr.path.is_ident(Ident::new("api", Span::call_site())) {
                use util::ApiItem;

                let attrs: util::ApiAttr = syn::parse2(attr.tts)?;

                for attr in attrs.items {
                    match attr {
                        ApiItem::Rename(to) => underscore_name = to,
                        //other => c_bail!(other.span(), "unsupported attribute on enum variant"),
                    }
                }
            } else {
                variant.attrs.push(attr);
            }
        }

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
