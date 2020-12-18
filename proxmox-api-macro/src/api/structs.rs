//! `#[api]` macro for `struct` types.
//!
//! This module implements struct handling.
//!
//! We distinguish between 3 types at the moment:
//! 1) Unit structs (`struct Foo;`),which don't do much really and aren't very useful for the API
//!    currently)
//! 2) Newtypes (`struct Foo(T)`), a 1-tuple, which is supposed to be a wrapper for a type `T` and
//!    therefore should implicitly deserialize/serialize to `T`. Currently we only support simple
//!    types for which we "know" the schema type used in the API.
//! 3) Object structs (`struct Foo { ... }`), which declare an `ObjectSchema`.

use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};

use anyhow::Error;

use proc_macro2::{Ident, Span, TokenStream};
use quote::quote_spanned;

use super::Schema;
use crate::api::{self, ObjectEntry, SchemaItem};
use crate::serde;
use crate::util::{self, FieldName, JSONObject, Maybe};

pub fn handle_struct(attribs: JSONObject, stru: syn::ItemStruct) -> Result<TokenStream, Error> {
    match &stru.fields {
        // unit structs, not sure about these?
        syn::Fields::Unit => handle_unit_struct(attribs, stru),
        syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            handle_newtype_struct(attribs, stru)
        }
        syn::Fields::Unnamed(fields) => bail!(
            fields.paren_token.span,
            "api macro does not support tuple structs"
        ),
        syn::Fields::Named(_) => handle_regular_struct(attribs, stru),
    }
}

fn get_struct_description(schema: &mut Schema, stru: &syn::ItemStruct) -> Result<(), Error> {
    if schema.description.is_none() {
        let (doc_comment, doc_span) = util::get_doc_comments(&stru.attrs)?;
        util::derive_descriptions(schema, None, &doc_comment, doc_span)?;
    }

    Ok(())
}

fn handle_unit_struct(attribs: JSONObject, stru: syn::ItemStruct) -> Result<TokenStream, Error> {
    // unit structs, not sure about these?

    let mut schema: Schema = if attribs.is_empty() {
        Schema::empty_object(Span::call_site())
    } else {
        attribs.try_into()?
    };

    get_struct_description(&mut schema, &stru)?;

    finish_schema(schema, &stru, &stru.ident)
}

fn finish_schema(
    schema: Schema,
    stru: &syn::ItemStruct,
    name: &Ident,
) -> Result<TokenStream, Error> {
    let schema = {
        let mut ts = TokenStream::new();
        schema.to_schema(&mut ts)?;
        ts
    };

    Ok(quote_spanned! { name.span() =>
        #stru
        impl #name {
            pub const API_SCHEMA: ::proxmox::api::schema::Schema = #schema;
        }
    })
}

fn handle_newtype_struct(attribs: JSONObject, stru: syn::ItemStruct) -> Result<TokenStream, Error> {
    // Ideally we could clone the contained item's schema, but this is "hard", so for now we assume
    // the contained type is a simple type.
    //
    // In order to support "specializing" an already existing type, we'd need to be able to
    // create "linked" schemas. We cannot do this purely via the macro.

    let mut schema: Schema = attribs.try_into()?;
    if let SchemaItem::Inferred(_span) = schema.item {
        // The schema has no `type` and we failed to guess it. Infer it from the contained field!

        let fields = match &stru.fields {
            syn::Fields::Unnamed(fields) => &fields.unnamed,

            // `handle_struct()` verified this!
            _ => panic!("handle_unit_struct on non-unit struct"),
        };
        // this is also part of `handle_struct()`'s verification!
        assert_eq!(
            fields.len(),
            1,
            "handle_unit_struct needs a struct with exactly 1 field"
        );

        // Now infer the type information:
        util::infer_type(&mut schema, &fields[0].ty)?;
    }

    get_struct_description(&mut schema, &stru)?;

    finish_schema(schema, &stru, &stru.ident)
}

fn handle_regular_struct(attribs: JSONObject, stru: syn::ItemStruct) -> Result<TokenStream, Error> {
    let mut schema: Schema = if attribs.is_empty() {
        Schema::empty_object(Span::call_site())
    } else {
        attribs.try_into()?
    };

    get_struct_description(&mut schema, &stru)?;

    // sanity check, first get us some quick by-name access to our fields:
    //
    // NOTE: We remove references we're "done with" and in the end fail with a list of extraneous
    // fields if there are any.
    let mut schema_fields: HashMap<String, &mut ObjectEntry> = HashMap::new();

    // We also keep a reference to the SchemaObject around since we derive missing fields
    // automatically.
    if let api::SchemaItem::Object(ref mut obj) = &mut schema.item {
        for field in obj.properties_mut() {
            schema_fields.insert(field.name.as_str().to_string(), field);
        }
    } else {
        error!(schema.span, "structs need an object schema");
    }

    let mut new_fields: Vec<ObjectEntry> = Vec::new();

    let container_attrs = serde::ContainerAttrib::try_from(&stru.attrs[..])?;

    let mut all_of_schemas = TokenStream::new();
    let mut to_remove = Vec::new();

    if let syn::Fields::Named(ref fields) = &stru.fields {
        for field in &fields.named {
            let attrs = serde::SerdeAttrib::try_from(&field.attrs[..])?;

            let (name, span) = {
                let ident: &Ident = field
                    .ident
                    .as_ref()
                    .ok_or_else(|| format_err!(field => "field without name?"))?;

                if let Some(renamed) = attrs.rename {
                    (renamed.into_str(), ident.span())
                } else if let Some(rename_all) = container_attrs.rename_all {
                    let name = rename_all.apply_to_field(&ident.to_string());
                    (name, ident.span())
                } else {
                    (ident.to_string(), ident.span())
                }
            };

            match schema_fields.remove(&name) {
                Some(field_def) => {
                    if attrs.flatten {
                        to_remove.push(name.clone());

                        if field_def.schema.description.is_explicit() {
                            error!(
                                field_def.name.span(),
                                "flattened field should not have a description, \
                                 it does not appear in serialized data as a field",
                            );
                        }

                        if field_def.optional {
                            error!(
                                field_def.name.span(),
                                "optional flattened fields are not supported"
                            );
                        }
                    }

                    handle_regular_field(field_def, field, false)?;

                    if attrs.flatten {
                        all_of_schemas.extend(quote::quote! {&});
                        field_def.schema.to_schema(&mut all_of_schemas)?;
                        all_of_schemas.extend(quote::quote! {,});
                    }
                }
                None => {
                    let mut field_def = ObjectEntry::new(
                        FieldName::new(name.clone(), span),
                        false,
                        Schema::blank(span),
                    );
                    handle_regular_field(&mut field_def, field, true)?;

                    if attrs.flatten {
                        all_of_schemas.extend(quote::quote! {&});
                        field_def.schema.to_schema(&mut all_of_schemas)?;
                        all_of_schemas.extend(quote::quote! {,});
                        to_remove.push(name.clone());
                    } else {
                        new_fields.push(field_def);
                    }
                }
            }
        }
    } else {
        panic!("handle_regular struct without named fields");
    };

    // now error out about all the fields not found in the struct:
    if !schema_fields.is_empty() {
        let bad_fields = util::join(", ", schema_fields.keys());
        error!(
            schema.span,
            "struct does not contain the following fields: {}", bad_fields
        );
    }

    if let api::SchemaItem::Object(ref mut obj) = &mut schema.item {
        // remove flattened fields
        for field in to_remove {
            if !obj.remove_property_by_ident(&field) {
                error!(
                    schema.span,
                    "internal error: failed to remove property {:?} from object schema", field,
                );
            }
        }

        // add derived fields
        obj.extend_properties(new_fields);
    } else {
        panic!("handle_regular_struct with non-object schema");
    }

    if all_of_schemas.is_empty() {
        finish_schema(schema, &stru, &stru.ident)
    } else {
        let name = &stru.ident;

        // take out the inner object schema's description
        let description = match schema.description.take().ok() {
            Some(description) => description,
            None => {
                error!(schema.span, "missing description on api type struct");
                syn::LitStr::new("<missing description>", schema.span)
            }
        };
        // and replace it with a "dummy"
        schema.description = Maybe::Derived(syn::LitStr::new(
            &format!("<INNER: {}>", description.value()),
            description.span(),
        ));

        // now check if it even has any fields
        let has_fields = match &schema.item {
            api::SchemaItem::Object(obj) => !obj.is_empty(),
            _ => panic!("object schema is not an object schema?"),
        };

        let (inner_schema, inner_schema_ref) = if has_fields {
            // if it does, we need to create an "inner" schema to merge into the AllOf schema
            let obj_schema = {
                let mut ts = TokenStream::new();
                schema.to_schema(&mut ts)?;
                ts
            };

            (
                quote_spanned!(name.span() =>
                    const INNER_API_SCHEMA: ::proxmox::api::schema::Schema = #obj_schema;
                ),
                quote_spanned!(name.span() => &Self::INNER_API_SCHEMA,),
            )
        } else {
            // otherwise it stays empty
            (TokenStream::new(), TokenStream::new())
        };

        Ok(quote_spanned!(name.span() =>
            #stru
            impl #name {
                #inner_schema
                pub const API_SCHEMA: ::proxmox::api::schema::Schema =
                    ::proxmox::api::schema::AllOfSchema::new(
                        #description,
                        &[
                            #inner_schema_ref
                            #all_of_schemas
                        ],
                    )
                    .schema();
            }
        ))
    }
}

/// Field handling:
///
/// For each field we derive the description from doc-attributes if available.
fn handle_regular_field(
    field_def: &mut ObjectEntry,
    field: &syn::Field,
    derived: bool, // whether this field was missing in the schema
) -> Result<(), Error> {
    let schema: &mut Schema = &mut field_def.schema;

    if schema.description.is_none() {
        let (doc_comment, doc_span) = util::get_doc_comments(&field.attrs)?;
        util::derive_descriptions(schema, None, &doc_comment, doc_span)?;
    }

    util::infer_type(schema, &field.ty)?;

    if is_option_type(&field.ty) {
        if derived {
            field_def.optional = true;
        } else if !field_def.optional {
            error!(&field.ty => "non-optional Option type?");
        }
    }

    Ok(())
}

/// Note that we cannot handle renamed imports at all here...
fn is_option_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(p) = ty {
        if p.qself.is_some() {
            return false;
        }
        let segs = &p.path.segments;
        match segs.len() {
            1 => return segs.last().unwrap().ident == "Option",
            2 => {
                return segs.first().unwrap().ident == "std"
                    && segs.last().unwrap().ident == "Option"
            }
            _ => return false,
        }
    }
    false
}
