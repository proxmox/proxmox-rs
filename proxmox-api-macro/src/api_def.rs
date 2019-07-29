use std::convert::TryFrom;

use proc_macro2::TokenStream;

use derive_builder::Builder;
use failure::{bail, Error};
use quote::{quote, ToTokens};

use super::parsing::{Expression, Object};

#[derive(Clone)]
pub enum CliMode {
    Disabled,
    ParseCli, // By default we try proxmox::cli::ParseCli
    FromStr,
    Function(syn::Expr),
}

impl Default for CliMode {
    fn default() -> Self {
        CliMode::ParseCli
    }
}

impl TryFrom<Expression> for CliMode {
    type Error = Error;
    fn try_from(expr: Expression) -> Result<Self, Error> {
        if expr.is_ident("FromStr") {
            return Ok(CliMode::FromStr);
        }

        if let Ok(value) = expr.is_lit_bool() {
            return Ok(if value.value {
                CliMode::ParseCli
            } else {
                CliMode::Disabled
            });
        }

        Ok(CliMode::Function(expr.expect_expr()?))
    }
}

impl CliMode {
    pub fn quote(&self, name: &proc_macro2::Ident) -> TokenStream {
        match self {
            CliMode::Disabled => quote! { None },
            CliMode::ParseCli => {
                quote! { Some(<#name as ::proxmox::api::cli::ParseCli>::parse_cli) }
            }
            CliMode::FromStr => quote! {
                Some(<#name as ::proxmox::api::cli::ParseCliFromStr>::parse_cli)
            },
            CliMode::Function(func) => quote! { Some(#func) },
        }
    }
}

#[derive(Builder)]
pub struct CommonTypeDefinition {
    pub description: syn::LitStr,
    #[builder(default)]
    pub cli: CliMode,
}

impl CommonTypeDefinition {
    fn builder() -> CommonTypeDefinitionBuilder {
        CommonTypeDefinitionBuilder::default()
    }

    pub fn from_object(obj: &mut Object) -> Result<Self, Error> {
        let mut def = Self::builder();

        if let Some(value) = obj.remove("description") {
            def.description(value.expect_lit_str()?);
        }
        if let Some(value) = obj.remove("cli") {
            def.cli(CliMode::try_from(value)?);
        }

        match def.build() {
            Ok(r) => Ok(r),
            Err(err) => bail!("{}", err),
        }
    }
}

#[derive(Builder)]
pub struct ParameterDefinition {
    #[builder(default)]
    pub default: Option<syn::Expr>,
    #[builder(default)]
    pub description: Option<syn::LitStr>,
    #[builder(default)]
    pub maximum: Option<syn::Expr>,
    #[builder(default)]
    pub minimum: Option<syn::Expr>,
    #[builder(default)]
    pub maximum_length: Option<syn::Expr>,
    #[builder(default)]
    pub minimum_length: Option<syn::Expr>,
    #[builder(default)]
    pub validate: Option<syn::Expr>,
}

impl ParameterDefinition {
    pub fn builder() -> ParameterDefinitionBuilder {
        Default::default()
    }

    pub fn from_object(obj: Object) -> Result<Self, Error> {
        let mut def = ParameterDefinition::builder();

        let obj_span = obj.span();
        for (key, value) in obj {
            match key.as_str() {
                "default" => {
                    def.default(Some(value.expect_expr()?));
                }
                "description" => {
                    def.description(Some(value.expect_lit_str()?));
                }
                "maximum" => {
                    def.maximum(Some(value.expect_expr()?));
                }
                "minimum" => {
                    def.minimum(Some(value.expect_expr()?));
                }
                "maximum_length" => {
                    def.maximum_length(Some(value.expect_expr()?));
                }
                "minimum_length" => {
                    def.minimum_length(Some(value.expect_expr()?));
                }
                "validate" => {
                    def.validate(Some(value.expect_expr()?));
                }
                other => c_bail!(key.span(), "invalid key in type definition: {}", other),
            }
        }

        match def.build() {
            Ok(r) => Ok(r),
            Err(err) => c_bail!(obj_span, "{}", err),
        }
    }

    pub fn from_expression(expr: Expression) -> Result<Self, Error> {
        let span = expr.span();
        match expr {
            Expression::Expr(syn::Expr::Lit(lit)) => match lit.lit {
                syn::Lit::Str(description) => Ok(ParameterDefinition::builder()
                    .description(Some(description))
                    .build()
                    .map_err(|e| c_format_err!(span, "{}", e))?),
                _ => c_bail!(span, "expected description or field definition"),
            },
            Expression::Object(obj) => ParameterDefinition::from_object(obj),
            _ => c_bail!(span, "expected description or field definition"),
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
