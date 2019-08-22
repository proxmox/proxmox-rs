use std::collections::HashMap;

use proc_macro2::{Delimiter, Group, Ident, Span, TokenStream, TokenTree};

use failure::{bail, Error};
use quote::quote;
use syn::{spanned::Spanned, Expr, Lit};

use crate::types::Name;

pub type RawTokenIter = proc_macro2::token_stream::IntoIter;
pub type TokenIter = std::iter::Peekable<RawTokenIter>;

pub fn optional_visibility(tokens: &mut TokenIter) -> Result<syn::Visibility, Error> {
    // peek:
    if let Some(TokenTree::Ident(ident)) = tokens.peek() {
        if ident != "pub" {
            return Ok(syn::Visibility::Inherited);
        }
    } else {
        return Ok(syn::Visibility::Inherited);
    }

    // consume:
    let ident = match tokens.next().unwrap() {
        TokenTree::Ident(ident) => ident,
        _ => unreachable!(),
    };

    // peek:
    let restriction = match tokens.peek() {
        Some(TokenTree::Group(_)) => true,
        _ => false,
    };

    let visibility = if restriction {
        // consume:
        match tokens.next().unwrap() {
            TokenTree::Group(g) => {
                quote! { #ident #g }
            }
            _ => unreachable!(),
        }
    } else {
        quote! { #ident }
    };

    use syn::parse::Parser;
    let parser = <syn::Visibility as syn::parse::Parse>::parse;

    Ok(parser.parse2(visibility)?)
}

pub fn match_keyword(
    span: Span,
    tokens: &mut TokenIter,
    keyword: &'static str,
) -> Result<Span, Error> {
    if let Some(tt) = tokens.next() {
        if let TokenTree::Ident(ident) = tt {
            if ident == keyword {
                return Ok(ident.span());
            }
        }
    }
    c_bail!(span, "expected `{}` keyword", keyword);
}

pub fn need_ident(before: Span, tokens: &mut TokenIter) -> Result<Ident, Error> {
    match tokens.next() {
        Some(TokenTree::Ident(ident)) => Ok(ident),
        Some(other) => c_bail!(other.span(), "expected ident"),
        None => c_bail!(before, "expected ident after this expression"),
    }
}

pub fn match_punct(span: Span, tokens: &mut TokenIter, punct: char) -> Result<Span, Error> {
    if let Some(tt) = tokens.next() {
        if let TokenTree::Punct(p) = tt {
            if p.as_char() == punct {
                return Ok(p.span());
            }
        }
    }
    c_bail!(span, "expected `{}` after this expression", punct);
}

pub fn need_group(tokens: &mut TokenIter, delimiter: Delimiter) -> Result<Group, Error> {
    if let Some(TokenTree::Group(group)) = tokens.next() {
        if group.delimiter() == delimiter {
            return Ok(group);
        }
    }
    bail!("expected group surrounded by {:?}", delimiter);
}

pub fn match_colon(tokens: &mut TokenIter) -> Result<(), Error> {
    match tokens.next() {
        Some(TokenTree::Punct(ref punct)) if punct.as_char() == ':' => Ok(()),
        Some(other) => c_bail!(other.span(), "expected colon"),
        None => bail!("colon expected"),
    }
}

pub fn match_colon2(span: Span, tokens: &mut TokenIter) -> Result<Span, Error> {
    match tokens.next() {
        Some(TokenTree::Punct(ref punct)) if punct.as_char() == ':' => Ok(punct.span()),
        Some(other) => c_bail!(other.span(), "expected colon"),
        None => c_bail!(span, "colon expected following this expression"),
    }
}

pub fn maybe_comma(tokens: &mut TokenIter) -> Result<bool, Error> {
    match tokens.next() {
        Some(TokenTree::Punct(ref punct)) if punct.as_char() == ',' => Ok(true),
        Some(other) => bail!("expected comma at {:?}", other.span()),
        None => Ok(false),
    }
}

pub fn need_comma(tokens: &mut TokenIter) -> Result<(), Error> {
    if !maybe_comma(tokens)? {
        bail!("comma expected");
    }
    Ok(())
}

// returns whther there was a comma
pub fn comma_or_end(tokens: &mut TokenIter) -> Result<(), Error> {
    if tokens.peek().is_some() {
        need_comma(tokens)?;
    }
    Ok(())
}

pub fn need_hyphenated_name(span: Span, tokens: &mut TokenIter) -> Result<syn::LitStr, Error> {
    let start = need_ident(span, &mut *tokens)?;
    finish_hyphenated_name(&mut *tokens, start)
}

pub fn finish_hyphenated_name(tokens: &mut TokenIter, name: Ident) -> Result<syn::LitStr, Error> {
    let span = name.span();
    let mut name = name.to_string();

    loop {
        if let Some(TokenTree::Punct(punct)) = tokens.peek() {
            if punct.as_char() == '-' {
                name.push('-');
                let _ = tokens.next();
            } else {
                break;
            }
        } else {
            break;
        }

        // after a hyphen we *need* another text:
        match tokens.next() {
            Some(TokenTree::Ident(ident)) => name.push_str(&ident.to_string()),
            Some(other) => bail!("expected name (possibly with hyphens): {:?}", other),
            None => bail!("unexpected end in name"),
        }
    }

    Ok(syn::LitStr::new(&name, span))
}

#[derive(Debug)]
pub enum Expression {
    Expr(Expr),
    Object(Object),
}

#[derive(Debug)]
pub struct Object {
    span: Span,
    map: HashMap<Name, Expression>,
}

impl std::ops::Deref for Object {
    type Target = HashMap<Name, Expression>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl std::ops::DerefMut for Object {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}

impl Object {
    pub fn new(span: Span) -> Self {
        Self {
            span,
            map: HashMap::new(),
        }
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

impl IntoIterator for Object {
    type Item = <HashMap<Name, Expression> as IntoIterator>::Item;
    type IntoIter = <HashMap<Name, Expression> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.map.into_iter()
    }
}

impl Expression {
    pub fn span(&self) -> Span {
        match self {
            Expression::Expr(expr) => expr.span(),
            Expression::Object(obj) => obj.span(),
        }
    }

    pub fn expect_lit_str(self) -> Result<syn::LitStr, Error> {
        match self {
            Expression::Expr(expr) => match expr {
                Expr::Lit(lit) => match lit.lit {
                    Lit::Str(lit) => Ok(lit),
                    other => bail!("expected string literal, got: {:?}", other),
                },
                other => bail!("expected string literal, got: {:?}", other),
            },
            other => c_bail!(other.span(), "expected string literal"),
        }
    }

    pub fn is_lit_bool(&self) -> Result<syn::LitBool, Error> {
        match self {
            Expression::Expr(expr) => match expr {
                Expr::Lit(lit) => match &lit.lit {
                    Lit::Bool(lit) => Ok(lit.clone()),
                    other => bail!("expected boolean literal, got: {:?}", other),
                },
                other => bail!("expected boolean literal, got: {:?}", other),
            },
            other => c_bail!(other.span(), "expected boolean literal"),
        }
    }

    pub fn expect_lit_bool(self) -> Result<syn::LitBool, Error> {
        match self {
            Expression::Expr(expr) => match expr {
                Expr::Lit(lit) => match lit.lit {
                    Lit::Bool(lit) => Ok(lit),
                    other => bail!("expected boolean literal, got: {:?}", other),
                },
                other => bail!("expected boolean literal, got: {:?}", other),
            },
            other => c_bail!(other.span(), "expected boolean literal"),
        }
    }

    pub fn expect_lit_bool_direct(self) -> Result<bool, Error> {
        Ok(self.expect_lit_bool()?.value)
    }

    pub fn expect_expr(self) -> Result<syn::Expr, Error> {
        match self {
            Expression::Expr(expr) => Ok(expr),
            other => c_bail!(other.span(), "expected expression, found {:?}", other),
        }
    }

    pub fn expect_path(self) -> Result<syn::Path, Error> {
        match self {
            Expression::Expr(Expr::Path(path)) => Ok(path.path),
            other => c_bail!(other.span(), "expected expression, found {:?}", other),
        }
    }

    pub fn expect_object(self) -> Result<Object, Error> {
        match self {
            Expression::Object(obj) => Ok(obj),
            other => c_bail!(other.span(), "expected object, found an expression"),
        }
    }

    pub fn expect_type(self) -> Result<syn::ExprPath, Error> {
        match self {
            Expression::Expr(expr) => match expr {
                Expr::Path(path) => Ok(path),
                other => bail!("expected a type name, got {:?}", other),
            },
            other => c_bail!(other.span(), "expected a type name, got {:?}", other),
        }
    }

    pub fn is_ident(&self, ident: &str) -> bool {
        match self {
            Expression::Expr(expr) => match expr {
                Expr::Path(path) => path.path.is_ident(Ident::new(ident, Span::call_site())),
                _ => false,
            },
            _ => false,
        }
    }
}

pub fn parse_object(tokens: TokenStream) -> Result<Object, Error> {
    let mut out = Object::new(tokens.span());
    let mut tokens = tokens.into_iter().peekable();

    while let Some(key) = parse_object_key(&mut tokens)? {
        let value = parse_object_value(&mut tokens, &key)?;

        if out.insert(key.clone(), value).is_some() {
            c_bail!(key.span(), "duplicate entry: {}", key.as_str());
        }
    }

    Ok(out)
}

fn parse_object_key(tokens: &mut TokenIter) -> Result<Option<Name>, Error> {
    let span = match tokens.peek() {
        Some(ref val) => val.span(),
        None => return Ok(None),
    };

    let key = need_ident_or_string(&mut *tokens)?;
    match_colon2(span, &mut *tokens)?;
    Ok(Some(key))
}

fn parse_object_value(tokens: &mut TokenIter, key: &Name) -> Result<Expression, Error> {
    let mut value_tokens = TokenStream::new();

    let mut first = true;
    loop {
        let token = match tokens.next() {
            Some(token) => token,
            None => {
                if first {
                    c_bail!(key => "missing value after key '{}'", key.as_str());
                }
                break;
            }
        };

        if first {
            first = false;
            if let TokenTree::Group(group) = token {
                let expr = parse_object(group.stream())?;
                comma_or_end(tokens)?;
                return Ok(Expression::Object(expr));
            }
        }

        match token {
            TokenTree::Punct(ref punct) if punct.as_char() == ',' => {
                // This is the end of the value!
                break;
            }
            _ => value_tokens.extend(vec![token]),
        }
    }

    let expr: Expr = syn::parse2(value_tokens)?;

    Ok(Expression::Expr(expr))
}

fn need_ident_or_string(tokens: &mut TokenIter) -> Result<Name, Error> {
    match tokens.next() {
        Some(TokenTree::Ident(ident)) => Ok(ident.into()),
        Some(TokenTree::Literal(literal)) => {
            let span = literal.span();
            match Lit::new(literal) {
                Lit::Str(value) => Ok(Name::new(value.value(), span)?),
                _ => bail!("expected ident or string as key: {:?}", span),
            }
        }
        Some(other) => bail!("expected an identifier or a string: {:?}", other.span()),
        None => bail!("ident expected"),
    }
}
