use std::collections::HashMap;

use proc_macro2::{Ident, TokenStream};

use derive_builder::Builder;
use failure::{bail, Error};
use quote::{quote, ToTokens};

use super::parsing::Value;

#[derive(Builder)]
pub struct ParameterDefinition {
    pub description: syn::LitStr,
    #[builder(default)]
    pub validate: Option<Ident>,
    #[builder(default)]
    pub minimum: Option<syn::Lit>,
    #[builder(default)]
    pub maximum: Option<syn::Lit>,
}

impl ParameterDefinition {
    pub fn builder() -> ParameterDefinitionBuilder {
        ParameterDefinitionBuilder::default()
    }

    pub fn from_object(obj: HashMap<String, Value>) -> Result<Self, Error> {
        let mut def = ParameterDefinition::builder();

        for (key, value) in obj {
            match key.as_str() {
                "description" => {
                    def.description(value.expect_lit_str()?);
                }
                "validate" => {
                    def.validate(Some(value.expect_ident()?));
                }
                "minimum" => {
                    def.minimum(Some(value.expect_lit()?));
                }
                "maximum" => {
                    def.maximum(Some(value.expect_lit()?));
                }
                other => bail!("invalid key in type definition: {}", other),
            }
        }

        match def.build() {
            Ok(r) => Ok(r),
            Err(err) => bail!("{}", err),
        }
    }

    pub fn add_verifiers(
        &self,
        name_str: &str,
        this: TokenStream,
        verifiers: &mut Vec<TokenStream>,
    ) {
        verifiers.push(match self.validate {
            Some(ref ident) => quote! { #ident(&#this)?; },
            None => quote! { ::proxmox::api::ApiType::verify(&#this)?; },
        });

        if let Some(ref lit) = self.minimum {
            let errstr = format!(
                "parameter '{}' out of range: (must be >= {})",
                name_str,
                lit.clone().into_token_stream().to_string(),
            );
            verifiers.push(quote! {
                if #this < #lit {
                    bail!("{}", #errstr);
                }
            });
        }

        if let Some(ref lit) = self.maximum {
            let errstr = format!(
                "parameter '{}' out of range: (must be <= {})",
                name_str,
                lit.clone().into_token_stream().to_string(),
            );
            verifiers.push(quote! {
                if #this > #lit {
                    bail!("{}", #errstr);
                }
            });
        }
    }
}
