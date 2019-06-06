use std::collections::HashMap;

use proc_macro2::{Delimiter, Ident, TokenStream, TokenTree};

use failure::{bail, Error};
use quote::quote;

use super::parsing::*;

pub fn router_macro(input: TokenStream) -> Result<TokenStream, Error> {
    let mut input = input.into_iter().peekable();

    let mut out = TokenStream::new();

    loop {
        if input.peek().is_none() {
            break;
        }

        match_keyword(&mut input, "static")?;
        let router_name = need_ident(&mut input)?;
        match_punct(&mut input, '=')?;
        let content = need_group(&mut input, Delimiter::Brace)?;

        let router = parse_router(content.stream().into_iter().peekable())?;
        let router = router.into_token_stream(Some(router_name));

        out.extend(router);

        match_punct(&mut input, ';')?;
    }

    Ok(out)
}

/// A sub-route entry. This represents subdirectories in a route entry.
///
/// This can either be a fixed set of directories, or a parameter name, in which case it matches
/// all directory names into the parameter of the specified name.
pub enum SubRoute {
    Directories(HashMap<String, Router>),
    Parameter(String, Box<Router>),
}

impl SubRoute {
    /// Create an ampty directories entry.
    fn directories() -> Self {
        SubRoute::Directories(HashMap::new())
    }

    /// Create a parameter entry with an empty default router.
    fn parameter(name: String) -> Self {
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
    Name(String),
    /// This component matches everything into a parameter. Eg. `bar` in `/foo/{bar}/baz`.
    Match(String),
}

/// A path is just a list of components.
type Path = Vec<Component>;

impl Router {
    /// Insert a new router at a specific path.
    ///
    /// Note that this does not allow replacing an already existing router node.
    fn insert(&mut self, path: Path, router: Router) -> Result<(), Error> {
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
                        SubRoute::Parameter(_param, _router) => {
                            bail!("subdirectory '{}' clashes with parameter matcher", name);
                        }
                    }
                }
                Component::Match(name) => {
                    let subroute = at.subroute.get_or_insert_with(|| {
                        created = true;
                        SubRoute::parameter(name.clone())
                    });
                    match subroute {
                        SubRoute::Directories(_) => {
                            bail!(
                                "parameter matcher '{}' clashes with existing directory",
                                name
                            );
                        }
                        SubRoute::Parameter(existing_name, router) => {
                            if name != *existing_name {
                                bail!(
                                    "paramter matcher '{}' clashes with existing name '{}'",
                                    name,
                                    existing_name,
                                );
                            }
                            at = router.as_mut();
                        }
                    }
                }
            }
        }

        if !created {
            bail!("tried to replace existing path in router");
        }
        std::mem::replace(at, router);
        Ok(())
    }

    fn into_token_stream(self, name: Option<Ident>) -> TokenStream {
        use std::iter::FromIterator;

        use proc_macro2::{Group, Literal, Punct, Spacing, Span};

        let mut out = vec![
            TokenTree::Ident(Ident::new("Router", Span::call_site())),
            TokenTree::Punct(Punct::new(':', Spacing::Joint)),
            TokenTree::Punct(Punct::new(':', Spacing::Alone)),
            TokenTree::Ident(Ident::new("new", Span::call_site())),
            TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
        ];

        fn add_method(out: &mut Vec<TokenTree>, name: &str, func_name: Ident) {
            out.push(TokenTree::Punct(Punct::new('.', Spacing::Alone)));
            out.push(TokenTree::Ident(Ident::new(name, Span::call_site())));
            out.push(TokenTree::Group(Group::new(
                Delimiter::Parenthesis,
                TokenStream::from_iter(vec![TokenTree::Ident(func_name)]),
            )));
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
                out.push(TokenTree::Punct(Punct::new('.', Spacing::Alone)));
                out.push(TokenTree::Ident(Ident::new(
                    "parameter_subdir",
                    Span::call_site(),
                )));
                let mut sub_route = TokenStream::from_iter(vec![
                    TokenTree::Literal(Literal::string(&name)),
                    TokenTree::Punct(Punct::new(',', Spacing::Alone)),
                ]);
                sub_route.extend(router.into_token_stream(None));
                out.push(TokenTree::Group(Group::new(
                    Delimiter::Parenthesis,
                    sub_route,
                )));
            }
            Some(SubRoute::Directories(hash)) => {
                for (name, router) in hash {
                    out.push(TokenTree::Punct(Punct::new('.', Spacing::Alone)));
                    out.push(TokenTree::Ident(Ident::new("subdir", Span::call_site())));
                    let mut sub_route = TokenStream::from_iter(vec![
                        TokenTree::Literal(Literal::string(&name)),
                        TokenTree::Punct(Punct::new(',', Spacing::Alone)),
                    ]);
                    sub_route.extend(router.into_token_stream(None));
                    out.push(TokenTree::Group(Group::new(
                        Delimiter::Parenthesis,
                        sub_route,
                    )));
                }
            }
        }

        if let Some(name) = name {
            let type_name = Ident::new(&format!("{}_TYPE", name.to_string()), name.span());
            let var_name = name;
            let router_expression = TokenStream::from_iter(out);

            quote! {
                #[allow(non_camel_case_types)]
                struct #type_name(std::cell::Cell<Option<Router>>, std::sync::Once);
                unsafe impl Sync for #type_name {}
                impl std::ops::Deref for #type_name {
                    type Target = Router;
                    fn deref(&self) -> &Self::Target {
                        self.1.call_once(|| unsafe {
                            self.0.set(Some(#router_expression));
                        });
                        unsafe {
                            (*self.0.as_ptr()).as_ref().unwrap()
                        }
                    }
                }
                static #var_name : #type_name = #type_name(
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
                let function = need_ident(&mut input)?;

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
    loop {
        match tokens.next() {
            None => bail!("expected path component"),
            Some(TokenTree::Group(group)) => {
                if group.delimiter() != Delimiter::Brace {
                    bail!("invalid path component: {:?}", group);
                }
                let name = need_hyphenated_name(&mut group.stream().into_iter().peekable())?;
                if !component.is_empty() {
                    path.push(Component::Name(component));
                    component = String::new();
                }
                path.push(Component::Match(name.into_string()));

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
                    if !component.is_empty() {
                        path.push(Component::Name(component));
                        component = String::new();
                    }
                    // `component` is cleared, we start the next one
                }
                other => bail!("invalid punctuation in path: {:?}", other),
            },
            Some(other) => bail!(
                "invalid path component, expected hyphen or slash: {:?}",
                other
            ),
        }
    }

    if !component.is_empty() {
        path.push(Component::Name(component));
    }

    Ok(path)
}
