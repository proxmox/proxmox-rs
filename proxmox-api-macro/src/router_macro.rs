use std::collections::HashMap;

use proc_macro2::{Delimiter, Ident, Span, TokenStream, TokenTree};

use failure::{bail, Error};
use quote::{quote, quote_spanned};
use syn::LitStr;

use super::parsing::*;

pub fn router_macro(input: TokenStream) -> Result<TokenStream, Error> {
    let mut input = input.into_iter().peekable();

    let mut out = TokenStream::new();

    loop {
        let mut at_span = match input.peek() {
            Some(ref val) => val.span(),
            None => break,
        };

        let public = optional_visibility(&mut input)?;

        at_span = match_keyword(at_span, &mut input, "static")?;
        let router_name = need_ident(at_span, &mut input)?;

        at_span = match_colon2(router_name.span(), &mut input)?;
        at_span = match_keyword(at_span, &mut input, "Router")?;
        at_span = match_punct(at_span, &mut input, '<')?;
        let body_type = need_ident(at_span, &mut input)?;
        at_span = match_punct(body_type.span(), &mut input, '>')?;

        at_span = match_punct(at_span, &mut input, '=')?;
        let content = need_group(&mut input, Delimiter::Brace)?;
        let _ = at_span;
        at_span = content.span();

        let router = parse_router(content.stream().into_iter().peekable())?;
        let router = router.into_token_stream(&body_type, Some((router_name, public)));

        //eprintln!("{}", router.to_string());

        out.extend(router);

        match_punct(at_span, &mut input, ';')?;
    }

    Ok(out)
}

/// A sub-route entry. This represents subdirectories in a route entry.
///
/// This can either be a fixed set of directories, or a parameter name, in which case it matches
/// all directory names into the parameter of the specified name.
pub enum SubRoute {
    Directories(HashMap<LitStr, Router>),
    Parameter(LitStr, Box<Router>),
    Wildcard(LitStr),
}

impl SubRoute {
    /// Create an ampty directories entry.
    fn directories() -> Self {
        SubRoute::Directories(HashMap::new())
    }

    /// Create a parameter entry with an empty default router.
    fn parameter(name: LitStr) -> Self {
        SubRoute::Parameter(name, Box::new(Router::default()))
    }
}

/// A set of operations for a specific directory entry, and an optional sub router.
#[derive(Default)]
pub struct Router {
    pub get: Option<Ident>,
    pub put: Option<Ident>,
    pub post: Option<Ident>,
    pub delete: Option<Ident>,
    pub subroute: Option<SubRoute>,
}

/// An entry for a router.
///
/// While parsing a router we either get a `path: router` key/value entry, or a
/// `method: function_name` entry.
enum Entry {
    /// This entry represents a path containing a sub router.
    Path(Path),
    /// This entry represents a method name.
    Method(Ident),
}

/// The components making up a path.
enum Component {
    /// This component is a fixed sub directory name. Eg. `foo` or `baz` in `/foo/{bar}/baz`.
    Name(LitStr),

    /// This component matches everything into a parameter. Eg. `bar` in `/foo/{bar}/baz`.
    Match(LitStr),

    /// Matches the rest of the path into a parameters
    Wildcard(LitStr),
}

/// A path is just a list of components.
type Path = Vec<Component>;

impl Router {
    /// Insert a new router at a specific path.
    ///
    /// Note that this does not allow replacing an already existing router node.
    fn insert(&mut self, path: Path, mut router: Router) -> Result<(), Error> {
        let mut at = self;
        let mut created = false;
        for component in path {
            created = false;
            match component {
                Component::Name(name) => {
                    let subroute = at.subroute.get_or_insert_with(SubRoute::directories);
                    match subroute {
                        SubRoute::Directories(hash) => {
                            at = hash.entry(name).or_insert_with(|| {
                                created = true;
                                Router::default()
                            });
                        }
                        SubRoute::Parameter(_, _) => {
                            bail!("subdir '{}' clashes with matched parameter", name.value());
                        }
                        SubRoute::Wildcard(_) => {
                            bail!("cannot add subdir '{}', it is already matched by a wildcard");
                        }
                    }
                }
                Component::Match(name) => {
                    let subroute = at.subroute.get_or_insert_with(|| {
                        created = true;
                        SubRoute::parameter(name.clone())
                    });
                    match subroute {
                        SubRoute::Parameter(existing_name, router) => {
                            if name != *existing_name {
                                bail!(
                                    "paramter matcher '{}' clashes with existing name '{}'",
                                    name.value(),
                                    existing_name.value(),
                                );
                            }
                            at = router.as_mut();
                        }
                        SubRoute::Directories(_) => {
                            bail!(
                                "parameter matcher '{}' clashes with existing directory",
                                name.value()
                            );
                        }
                        SubRoute::Wildcard(_) => {
                            bail!("parameter matcher '{}' clashes with wildcard", name.value());
                        }
                    }
                }
                Component::Wildcard(name) => {
                    if at.subroute.is_some() {
                        bail!("wildcard clashes with existing subdirectory");
                    }
                    created = true;
                    if router.subroute.is_some() {
                        bail!("wildcard sub router cannot have subdirectories!");
                    }
                    router.subroute = Some(SubRoute::Wildcard(name.clone()));
                }
            }
        }

        if !created {
            bail!("tried to replace existing path in router");
        }
        std::mem::replace(at, router);
        Ok(())
    }

    fn into_token_stream(
        self,
        body_type: &Ident,
        name: Option<(Ident, syn::Visibility)>,
    ) -> TokenStream {
        use std::iter::FromIterator;

        let mut out = quote_spanned! {
            body_type.span() => ::proxmox::api::Router::<#body_type>::new()
        };

        fn add_method(out: &mut TokenStream, name: &'static str, func_name: Ident) {
            let name = Ident::new(name, func_name.span());
            out.extend(quote! {
                .#name(#func_name)
            });
        }

        if let Some(method) = self.get {
            add_method(&mut out, "get", method);
        }
        if let Some(method) = self.put {
            add_method(&mut out, "put", method);
        }
        if let Some(method) = self.post {
            add_method(&mut out, "post", method);
        }
        if let Some(method) = self.delete {
            add_method(&mut out, "delete", method);
        }

        match self.subroute {
            None => (),
            Some(SubRoute::Parameter(name, router)) => {
                let router = router.into_token_stream(body_type, None);
                out.extend(quote! {
                    .parameter_subdir(#name, #router)
                });
            }
            Some(SubRoute::Directories(hash)) => {
                for (name, router) in hash {
                    let router = router.into_token_stream(body_type, None);
                    out.extend(quote! {
                        .subdir(#name, #router)
                    });
                }
            }
            Some(SubRoute::Wildcard(name)) => {
                out.extend(quote! {
                    .wildcard(#name)
                });
            }
        }

        if let Some((name, vis)) = name {
            let type_name = Ident::new(&format!("{}_TYPE", name.to_string()), name.span());
            let var_name = name;
            let router_expression = TokenStream::from_iter(out);

            quote! {
                #[allow(non_camel_case_types)]
                #vis struct #type_name(
                    std::cell::Cell<Option<::proxmox::api::Router<#body_type>>>,
                    std::sync::Once,
                );
                unsafe impl Sync for #type_name {}
                impl std::ops::Deref for #type_name {
                    type Target = ::proxmox::api::Router<#body_type>;
                    fn deref(&self) -> &Self::Target {
                        self.1.call_once(|| unsafe {
                            self.0.set(Some(#router_expression));
                        });
                        unsafe {
                            (*self.0.as_ptr()).as_ref().unwrap()
                        }
                    }
                }
                #vis static #var_name : #type_name = #type_name(
                    std::cell::Cell::new(None),
                    std::sync::Once::new(),
                );
            }
        } else {
            TokenStream::from_iter(out)
        }
    }
}

fn parse_router(mut input: TokenIter) -> Result<Router, Error> {
    let mut router = Router::default();
    loop {
        match parse_entry_key(&mut input)? {
            Some(Entry::Method(name)) => {
                let function = need_ident(name.span(), &mut input)?;

                let method_ptr = match name.to_string().as_str() {
                    "GET" => &mut router.get,
                    "PUT" => &mut router.put,
                    "POST" => &mut router.post,
                    "DELETE" => &mut router.delete,
                    other => bail!("not a valid method name: {}", other.to_string()),
                };

                if method_ptr.is_some() {
                    bail!("duplicate method entry: {}", name.to_string());
                }

                *method_ptr = Some(function);
            }
            Some(Entry::Path(path)) => {
                let sub_content = need_group(&mut input, Delimiter::Brace)?;
                let sub_router = parse_router(sub_content.stream().into_iter().peekable())?;
                router.insert(path, sub_router)?;
            }
            None => break,
        }
        comma_or_end(&mut input)?;
    }
    Ok(router)
}

fn parse_entry_key(tokens: &mut TokenIter) -> Result<Option<Entry>, Error> {
    match tokens.next() {
        None => Ok(None),
        Some(TokenTree::Punct(ref punct)) if punct.as_char() == '/' => {
            Ok(Some(Entry::Path(parse_path_name(tokens)?)))
        }
        Some(TokenTree::Ident(ident)) => {
            match_colon(tokens)?;
            Ok(Some(Entry::Method(ident)))
        }
        Some(other) => bail!("invalid router entry: {:?}", other),
    }
}

fn parse_path_name(tokens: &mut TokenIter) -> Result<Path, Error> {
    let mut path = Path::new();
    let mut component = String::new();
    let mut span = None;

    fn push_component(path: &mut Path, component: &mut String, span: &mut Option<Span>) {
        if !component.is_empty() {
            path.push(Component::Name(LitStr::new(
                &component,
                span.take().unwrap(),
            )));
            component.clear();
        }
    };

    loop {
        match tokens.next() {
            None => bail!("expected path component"),
            Some(TokenTree::Group(group)) => {
                if group.delimiter() != Delimiter::Brace {
                    bail!("invalid path component: {:?}", group);
                }
                let name = need_hyphenated_name(
                    group.span(),
                    &mut group.stream().into_iter().peekable(),
                )?;
                push_component(&mut path, &mut component, &mut span);
                path.push(Component::Match(name));

                // Now:
                //     `component` is empty
                // Next tokens:
                //     `:` (and we're done)
                //     `/` (and we start the next component)
            }
            Some(TokenTree::Punct(ref punct)) if punct.as_char() == ':' => {
                if !component.is_empty() {
                    // this only happens when we hit the '-' case
                    bail!("name must not end with a hyphen");
                }
                break;
            }
            Some(TokenTree::Ident(ident)) => {
                component.push_str(&ident.to_string());
                if span.is_none() {
                    span = Some(ident.span());
                }

                // Now:
                //     `component` is partially or fully filled
                // Next tokens:
                //     `:` (and we're done)
                //     `/` (and we start the next component)
                //     `-` (the component name is not finished yet)
            }
            Some(TokenTree::Literal(literal)) => {
                let text = literal.to_string();
                let litspan = literal.span();
                match syn::Lit::new(literal) {
                    syn::Lit::Int(_) => {
                        component.push_str(&text);
                        if span.is_none() {
                            span = Some(litspan);
                        }
                    }
                    other => {
                        bail!("invalid literal path component: {:?}", other);
                    }
                }
                // Same case as the Ident case above:
                // Now:
                //     `component` is partially or fully filled
                // Next tokens:
                //     `:` (and we're done)
                //     `/` (and we start the next component)
                //     `-` (the component name is not finished yet)
            }
            Some(other) => bail!("invalid path component: {:?}", other),
        }

        // there may be hyphens here, but we don't allow space separated paths or other symbols
        match tokens.next() {
            None => break,
            Some(TokenTree::Punct(punct)) => match punct.as_char() {
                ':' => break, // okay in both cases
                '-' => {
                    if component.is_empty() {
                        bail!("unexpected hyphen after parameter matcher");
                    }
                    component.push('-');
                    // `component` is partially filled, we need more
                }
                '/' => {
                    push_component(&mut path, &mut component, &mut span);
                    // `component` is cleared, we start the next one
                }
                '*' => {
                    // must be the last component, after a matcher
                    if !component.is_empty() {
                        bail!("wildcard must be the final matcher");
                    }
                    if let Some(Component::Match(name)) = path.pop() {
                        path.push(Component::Wildcard(name));
                        match_colon(&mut *tokens)?;
                        break;
                    }
                    bail!("asterisk only allowed at the end of a match pattern");
                }
                other => bail!("invalid punctuation in path: {:?}", other),
            },
            Some(other) => bail!(
                "invalid path component, expected hyphen or slash: {:?}",
                other
            ),
        }
    }

    push_component(&mut path, &mut component, &mut span);

    Ok(path)
}
