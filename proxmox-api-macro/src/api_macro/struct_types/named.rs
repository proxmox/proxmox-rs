//! Handler for named struct types `struct Foo { name: T, ... }`.

use proc_macro2::{Ident, Span, TokenStream};

use failure::{bail, Error};
use quote::quote_spanned;
use syn::spanned::Spanned;

use crate::api_def::{CommonTypeDefinition, ParameterDefinition};
use crate::parsing::Object;

use super::StructField;

pub fn handle_struct_named(
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

    let impl_verify = super::struct_fields_impl_verify(item.span(), &fields)?;
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

                const FIELDS: &[&str] = &[ #field_name_str_list ];
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
                    static DEF: <#field_ty as ::proxmox::api::meta::OrDefault>::Output = #default;
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
