use std::collections::HashMap;

use proc_macro2::{Delimiter, Group, Ident, Span, TokenStream, TokenTree};

use failure::{bail, Error};
use syn::{Expr, Lit};

pub type RawTokenIter = proc_macro2::token_stream::IntoIter;
pub type TokenIter = std::iter::Peekable<RawTokenIter>;

pub fn match_keyword(tokens: &mut TokenIter, keyword: &'static str) -> Result<(), Error> {
    if let Some(tt) = tokens.next() {
        if let TokenTree::Ident(ident) = tt {
            if ident.to_string() == keyword {
                return Ok(());
            }
        }
    }
    bail!("expected `{}` keyword", keyword);
}

pub fn need_ident(tokens: &mut TokenIter) -> Result<Ident, Error> {
    match tokens.next() {
        Some(TokenTree::Ident(ident)) => Ok(ident),
        other => bail!("expected ident: {:?}", other),
    }
}

pub fn match_punct(tokens: &mut TokenIter, punct: char) -> Result<(), Error> {
    if let Some(tt) = tokens.next() {
        if let TokenTree::Punct(p) = tt {
            if p.as_char() == punct {
                return Ok(());
            }
        }
    }
    bail!("expected `{}`", punct);
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
        Some(other) => bail!("expected colon at {:?}", other.span()),
        None => bail!("colon expected"),
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

/// A more relaxed version of Ident which allows hyphens.
pub struct Name(String, Span);

impl Name {
    pub fn new(name: String, span: Span) -> Result<Self, Error> {
        let beg = name.as_bytes()[0];
        if !(beg.is_ascii_alphanumeric() || beg == b'_')
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            bail!("`{}` is not a valid name", name);
        }
        Ok(Self(name, span))
    }

    pub fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl From<Ident> for Name {
    fn from(ident: Ident) -> Name {
        Name(ident.to_string(), ident.span())
    }
}

pub fn need_hyphenated_name(tokens: &mut TokenIter) -> Result<syn::LitStr, Error> {
    let start = need_ident(&mut *tokens)?;
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
    Object(HashMap<String, Expression>),
}

impl Expression {
    pub fn expect_lit_str(self) -> Result<syn::LitStr, Error> {
        match self {
            Expression::Expr(expr) => match expr {
                Expr::Lit(lit) => match lit.lit {
                    Lit::Str(lit) => Ok(lit),
                    other => bail!("expected string literal, got: {:?}", other),
                }
                other => bail!("expected string literal, got: {:?}", other),
            }
            _ => bail!("expected string literal"),
        }
    }

    pub fn expect_lit_bool(self) -> Result<syn::LitBool, Error> {
        match self {
            Expression::Expr(expr) => match expr {
                Expr::Lit(lit) => match lit.lit {
                    Lit::Bool(lit) => Ok(lit),
                    other => bail!("expected boolean literal, got: {:?}", other),
                }
                other => bail!("expected boolean literal, got: {:?}", other),
            }
            _ => bail!("expected boolean literal"),
        }
    }

    pub fn expect_expr(self) -> Result<syn::Expr, Error> {
        match self {
            Expression::Expr(expr) => Ok(expr),
            _ => bail!("expected expression, found {:?}", self),
        }
    }

    pub fn expect_object(self) -> Result<HashMap<String, Expression>, Error> {
        match self {
            Expression::Object(obj) => Ok(obj),
            _ => bail!("expected object, found an expression"),
        }
    }

    pub fn expect_type(self) -> Result<syn::ExprPath, Error> {
        match self {
            Expression::Expr(expr) => match expr {
                Expr::Path(path) => Ok(path),
                other => bail!("expected a type name, got {:?}", other),
            }
            _ => bail!("expected a type name, got {:?}", self),
        }
    }
}

pub fn parse_object2(tokens: TokenStream) -> Result<HashMap<String, Expression>, Error> {
    let mut tokens = tokens.into_iter().peekable();
    let mut out = HashMap::new();

    loop {
        let key = match parse_object_key(&mut tokens)? {
            Some(key) => key,
            None => break,
        };
        let key_name = key.to_string();

        let value = parse_object_value(&mut tokens, &key_name)?;

        if out.insert(key_name.clone(), value).is_some() {
            bail!("duplicate entry: {}", key_name);
        }
    }

    Ok(out)
}

fn parse_object_key(tokens: &mut TokenIter) -> Result<Option<Name>, Error> {
    if tokens.peek().is_none() {
        return Ok(None);
    }

    let key = need_ident_or_string(&mut *tokens)?;
    match_colon(&mut *tokens)?;
    Ok(Some(key))
}

fn parse_object_value(tokens: &mut TokenIter, key: &str) -> Result<Expression, Error> {
    let mut value_tokens = TokenStream::new();

    let mut first = true;
    loop {
        let token = match tokens.next() {
            Some(token) => token,
            None => {
                if first {
                    bail!("missing value after key '{}'", key);
                }
                break;
            }
        };

        if first {
            first = false;
            if let TokenTree::Group(group) = token {
                let expr = parse_object2(group.stream())?;
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
        Some(other) => bail!(
            "expected an identifier or a string: {:?}",
            other.span()
        ),
        None => bail!("ident expected"),
    }
}
