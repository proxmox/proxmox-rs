//! This module provides a router used for http servers.

use std::collections::HashMap;

use serde_json::Value;

use super::ApiMethodInfo;

/// This enum specifies what to do when a subdirectory is requested from the current router.
///
/// For plain subdirectories a `Directories` entry is used.
///
/// When subdirectories are supposed to be passed as a `String` parameter to methods beneath the
/// current directory, a `Parameter` entry is used. Note that the parameter name is fixed at this
/// point, so all method calls beneath will receive a parameter ot that particular name.
pub enum SubRoute<Body: 'static> {
    /// Call this router for any further subdirectory paths, and provide the relative path via the
    /// given parameter.
    Wildcard(&'static str),

    /// This is used for plain subdirectories.
    Directories(HashMap<&'static str, Router<Body>>),

    /// Match subdirectories as the given parameter name to the underlying router.
    Parameter(&'static str, Box<Router<Body>>),
}

/// A router is a nested structure. On the one hand it contains HTTP method entries (`GET`, `PUT`,
/// ...), and on the other hand it contains sub directories. In some cases we want to match those
/// sub directories as parameters, so the nesting uses a `SubRoute` `enum` representing which of
/// the two is the case.
#[derive(Default)]
pub struct Router<Body: 'static> {
    /// The `GET` http method.
    pub get: Option<&'static dyn ApiMethodInfo<Body>>,

    /// The `PUT` http method.
    pub put: Option<&'static dyn ApiMethodInfo<Body>>,

    /// The `POST` http method.
    pub post: Option<&'static dyn ApiMethodInfo<Body>>,

    /// The `DELETE` http method.
    pub delete: Option<&'static dyn ApiMethodInfo<Body>>,

    /// Specifies the behavior of sub directories. See [`SubRoute`].
    pub subroute: Option<SubRoute<Body>>,
}

impl<Body> Router<Body>
where
    Self: Default,
{
    /// Create a new empty router.
    pub fn new() -> Self {
        Self::default()
    }

    /// Lookup a path in the router. Note that this returns a tuple: the router we ended up on
    /// (providing methods and subdirectories available for the given path), and optionally a json
    /// value containing all the matched parameters ([`SubRoute::Parameter`] subdirectories).
    pub fn lookup<T: AsRef<str>>(&self, path: T) -> Option<(&Self, Option<Value>)> {
        self.lookup_do(path.as_ref())
    }

    // The actual implementation taking the parameter as &str
    fn lookup_do(&self, path: &str) -> Option<(&Self, Option<Value>)> {
        let mut matched_params = None;
        let mut matched_wildcard: Option<String> = None;

        let mut this = self;
        for component in path.split('/') {
            if let Some(ref mut relative_path) = matched_wildcard {
                relative_path.push('/');
                relative_path.push_str(component);
                continue;
            }
            if component.is_empty() {
                // `foo//bar` or the first `/` in `/foo`
                continue;
            }
            this = match &this.subroute {
                Some(SubRoute::Wildcard(_)) => {
                    matched_wildcard = Some(component.to_string());
                    continue;
                }
                Some(SubRoute::Directories(subdirs)) => subdirs.get(component)?,
                Some(SubRoute::Parameter(param_name, router)) => {
                    let previous = matched_params
                        .get_or_insert_with(serde_json::Map::new)
                        .insert(param_name.to_string(), Value::String(component.to_string()));
                    if previous.is_some() {
                        panic!("API contains the same parameter twice in route");
                    }
                    &*router
                }
                None => return None,
            };
        }

        if let Some(SubRoute::Wildcard(param_name)) = &this.subroute {
            matched_params
                .get_or_insert_with(serde_json::Map::new)
                .insert(
                    param_name.to_string(),
                    Value::String(matched_wildcard.unwrap_or(String::new())),
                );
        }

        Some((this, matched_params.map(Value::Object)))
    }

    /// Builder method to provide a `GET` method info.
    pub fn get<I>(mut self, method: &'static I) -> Self
    where
        I: ApiMethodInfo<Body>,
    {
        self.get = Some(method);
        self
    }

    /// Builder method to provide a `PUT` method info.
    pub fn put<I>(mut self, method: &'static I) -> Self
    where
        I: ApiMethodInfo<Body>,
    {
        self.put = Some(method);
        self
    }

    /// Builder method to provide a `POST` method info.
    pub fn post<I>(mut self, method: &'static I) -> Self
    where
        I: ApiMethodInfo<Body>,
    {
        self.post = Some(method);
        self
    }

    /// Builder method to provide a `DELETE` method info.
    pub fn delete<I>(mut self, method: &'static I) -> Self
    where
        I: ApiMethodInfo<Body>,
    {
        self.delete = Some(method);
        self
    }

    /// Builder method to make this router match the next subdirectory into a parameter.
    ///
    /// This is supposed to be used statically (via `lazy_static!), therefore we panic if we
    /// already have a subdir entry!
    pub fn parameter_subdir(mut self, parameter_name: &'static str, router: Router<Body>) -> Self {
        if self.subroute.is_some() {
            panic!("match_parameter can only be used once and without sub directories");
        }
        self.subroute = Some(SubRoute::Parameter(parameter_name, Box::new(router)));
        self
    }

    /// Builder method to add a regular directory entry to this router.
    ///
    /// This is supposed to be used statically (via `lazy_static!), therefore we panic if we
    /// already have a subdir entry!
    pub fn subdir(mut self, dir_name: &'static str, router: Router<Body>) -> Self {
        let previous = match self.subroute {
            Some(SubRoute::Directories(ref mut map)) => map.insert(dir_name, router),
            None => {
                let mut map = HashMap::new();
                map.insert(dir_name, router);
                self.subroute = Some(SubRoute::Directories(map));
                None
            }
            _ => panic!("subdir and match_parameter are mutually exclusive"),
        };
        if previous.is_some() {
            panic!("duplicate subdirectory: {}", dir_name);
        }
        self
    }

    /// Builder method to match the rest of the path into a parameter.
    ///
    /// This is supposed to be used statically (via `lazy_static!), therefore we panic if we
    /// already have a subdir entry!
    pub fn wildcard(mut self, path_parameter_name: &'static str) -> Self {
        if self.subroute.is_some() {
            panic!("'wildcard' and other sub routers are mutually exclusive");
        }

        self.subroute = Some(SubRoute::Wildcard(path_parameter_name));

        self
    }
}
