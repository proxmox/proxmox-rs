use std::collections::HashMap;

use proc_macro2::{Delimiter, Group, Ident, Span, TokenStream, TokenTree};

use failure::{bail, Error};
use syn::Lit;

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

// parse an object notation:
// object := '{' [ member * ] '}'
// member := <ident> ':' <member_value>
// member_value := [ "optional" ] ( <ident> | <literal> | <object> )
#[derive(Debug)]
pub enum Value {
    //Ident(Ident),                 // eg. `string` or `integer`
    //Description(syn::LitStr),       // eg. `"some text"`
    Ident(Ident),       // eg. `foo`, for referencing stuff, may become `expression`?
    Literal(syn::Lit),  // eg. `123`
    Negative(syn::Lit), // eg. `-123`
    Object(HashMap<String, Value>), // eg. `{ key: value }`
}

impl Value {
    pub fn expect_lit(self) -> Result<syn::Lit, Error> {
        match self {
            Value::Literal(lit) => Ok(lit),
            other => bail!("expected string literal, got: {:?}", other),
        }
    }

    pub fn expect_lit_str(self) -> Result<syn::LitStr, Error> {
        match self {
            Value::Literal(syn::Lit::Str(lit)) => Ok(lit),
            Value::Literal(other) => bail!("expected string literal, got: {:?}", other),
            other => bail!("expected string literal, got: {:?}", other),
        }
    }

    pub fn expect_ident(self) -> Result<Ident, Error> {
        match self {
            Value::Ident(ident) => Ok(ident),
            other => bail!("expected ident, got: {:?}", other),
        }
    }

    pub fn expect_object(self) -> Result<HashMap<String, Value>, Error> {
        match self {
            Value::Object(obj) => Ok(obj),
            other => bail!("expected ident, got: {:?}", other),
        }
    }

    pub fn expect_lit_bool(self) -> Result<syn::LitBool, Error> {
        match self {
            Value::Literal(syn::Lit::Bool(lit)) => Ok(lit),
            Value::Literal(other) => bail!("expected booleanliteral, got: {:?}", other),
            other => bail!("expected boolean literal, got: {:?}", other),
        }
    }
}

pub fn parse_object(tokens: TokenStream) -> Result<HashMap<String, Value>, Error> {
    let mut tokens = tokens.into_iter().peekable();
    let mut out = HashMap::new();

    loop {
        if tokens.peek().is_none() {
            break;
        }

        let key = need_ident_or_string(&mut tokens)?;
        match_colon(&mut tokens)?;

        let key_name = key.to_string();

        let member = match tokens.next() {
            Some(TokenTree::Group(group)) => {
                if group.delimiter() == Delimiter::Brace {
                    Value::Object(parse_object(group.stream())?)
                } else {
                    bail!("invalid group delimiter: {:?}", group.delimiter());
                }
            }
            Some(TokenTree::Punct(ref punct)) if punct.as_char() == '-' => {
                if let Some(TokenTree::Literal(literal)) = tokens.next() {
                    let lit = Lit::new(literal);
                    match lit {
                        Lit::Int(_) | Lit::Float(_) => Value::Negative(lit),
                        _ => bail!("expected literal after unary minus"),
                    }
                } else {
                    bail!("expected literal value");
                }
            }
            Some(TokenTree::Literal(literal)) => Value::Literal(Lit::new(literal)),
            Some(TokenTree::Ident(ident)) => Value::Ident(ident),
            Some(other) => bail!("expected member value at {}", other),
            None => bail!("missing member value after {}", key_name),
        };

        if out.insert(key_name.clone(), member).is_some() {
            bail!("duplicate entry: {}", key_name);
        }

        comma_or_end(&mut tokens)?;
    }

    Ok(out)
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
            "expected colon after key in api definition at {:?}",
            other.span()
        ),
        None => bail!("ident expected"),
    }
}
