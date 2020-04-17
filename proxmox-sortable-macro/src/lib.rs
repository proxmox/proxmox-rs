extern crate proc_macro;
extern crate proc_macro2;

use std::iter::FromIterator;
use std::mem;

use anyhow::Error;

use proc_macro::TokenStream as TokenStream_1;
use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit_mut::VisitMut;
use syn::Ident;

macro_rules! format_err {
    ($span:expr => $($msg:tt)*) => { syn::Error::new_spanned($span, format!($($msg)*)) };
    ($span:expr, $($msg:tt)*) => { syn::Error::new($span, format!($($msg)*)) };
}

//macro_rules! bail {
//    ($span:expr => $($msg:tt)*) => { return Err(format_err!($span => $($msg)*).into()) };
//    ($span:expr, $($msg:tt)*) => { return Err(format_err!($span, $($msg)*).into()) };
//}

fn handle_error(mut item: TokenStream, data: Result<TokenStream, Error>) -> TokenStream {
    match data {
        Ok(output) => output,
        Err(err) => match err.downcast::<syn::Error>() {
            Ok(err) => {
                item.extend(err.to_compile_error());
                item
            }
            Err(err) => panic!("error in sortable macro: {}", err),
        },
    }
}

/// Enable the `sorted!` expression-position macro in a statement.
#[proc_macro_attribute]
pub fn sortable(_attr: TokenStream_1, item: TokenStream_1) -> TokenStream_1 {
    let item: TokenStream = item.into();
    handle_error(item.clone(), sortable_do(item)).into()
}

struct SortedData;

impl VisitMut for SortedData {
    fn visit_expr_macro_mut(&mut self, i: &mut syn::ExprMacro) {
        if i.mac.path.is_ident("sorted") {
            let span = i.mac.path.span();
            i.mac.path.segments = Punctuated::new();
            i.mac.path.segments.push(syn::PathSegment {
                ident: Ident::new("identity", span),
                arguments: Default::default(),
            });

            let tokens = mem::replace(&mut i.mac.tokens, TokenStream::new());
            i.mac.tokens = handle_error(tokens.clone(), sort_data(tokens));
        }
        // and recurse:
        self.visit_macro_mut(&mut i.mac)
    }
}

fn sortable_do(item: TokenStream) -> Result<TokenStream, Error> {
    let mut item: syn::Item = syn::parse2(item)?;
    SortedData.visit_item_mut(&mut item);
    Ok(quote!(#item))
}

fn sort_data(data: TokenStream) -> Result<TokenStream, Error> {
    let mut array: syn::ExprArray = syn::parse2(data)?;
    let span = array.span();

    let mut fields: Vec<syn::Expr> = mem::replace(&mut array.elems, Punctuated::new())
        .into_iter()
        .collect();

    let mut err = None;
    fields.sort_by(|a, b| {
        if err.is_some() {
            return std::cmp::Ordering::Equal;
        }

        use syn::{Expr, Lit};
        match (a, b) {
            // We can sort an array of literals:
            (Expr::Lit(a), Expr::Lit(b)) => match (&a.lit, &b.lit) {
                (Lit::Str(a), Lit::Str(b)) => return a.value().cmp(&b.value()),
                _ => err = Some(format_err!(span, "can only sort by string literals!")),
            },

            // We can sort an array of tuples where the first element is a literal:
            (Expr::Tuple(a), Expr::Tuple(b)) => match (a.elems.first(), b.elems.first()) {
                (Some(Expr::Lit(a)), Some(Expr::Lit(b))) => match (&a.lit, &b.lit) {
                    (Lit::Str(a), Lit::Str(b)) => return a.value().cmp(&b.value()),
                    _ => err = Some(format_err!(span, "can only sort by string literals!")),
                },
                _ => {
                    err = Some(format_err!(
                        span,
                        "can only sort tuples starting with literals!"
                    ))
                }
            },
            _ => err = Some(format_err!(span, "don't know how to sort this data!")),
        }
        std::cmp::Ordering::Equal
    });

    if let Some(err) = err {
        return Err(err.into());
    }

    array.elems = Punctuated::from_iter(fields);

    Ok(quote!(#array))
}
