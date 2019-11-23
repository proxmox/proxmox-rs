use std::collections::HashMap;

use hyper::Method;

use crate::ApiMethod;

/// Lookup table to child `Router`s
///
/// Stores a sorted list of `(name, router)` tuples:
///
/// - `name`: The name of the subdir
/// - `router`: The router for this subdir
///
/// **Note:** The list has to be storted by name, because we use a binary
/// search to find items.
///
/// This is a workaround unless RUST can const_fn `Hash::new()`
pub type SubdirMap = &'static [(&'static str, &'static Router)];

/// Clasify different types of routers
pub enum SubRoute {
    //Hash(HashMap<String, Router>),
    /// Router with static lookup map.
    ///
    /// The first path element is used to lookup a new
    /// router with `SubdirMap`. If found, the rest of the path is
    /// passed to that router.
    Map(SubdirMap),
    /// Router that always match the first path element
    ///
    /// The matched path element is stored as parameter
    /// `param_name`. The rest of the path is matched using the
    /// `router`.
    MatchAll {
        router: &'static Router,
        param_name: &'static str,
    },
}

/// Macro to create an ApiMethod to list entries from SubdirMap
#[macro_export]
macro_rules! list_subdirs_api_method {
    ($map:expr) => {
        $crate::ApiMethod::new(
            &$crate::ApiHandler::Sync( & |_, _, _| {
                let index = ::serde_json::json!(
                    $map.iter().map(|s| ::serde_json::json!({ "subdir": s.0}))
                        .collect::<Vec<::serde_json::Value>>()
                );
                Ok(index)
            }),
            &$crate::schema::ObjectSchema::new("Directory index.", &[]).additional_properties(true)
        )
    }
}

/// Define APIs with routing information
///
/// REST APIs use hierarchical paths to identify resources. A path
/// consists of zero or more components, separated by `/`. A `Router`
/// is a simple data structure to define such APIs. Each `Router` is
/// responsible for a specific path, and may define `ApiMethod`s for
/// different HTTP requests (GET, PUT, POST, DELETE). If the path
/// contains more elements, `subroute` is used to find the correct
/// endpoint.
///
/// Routers are meant to be build a compile time, and you can use
/// all `const fn(mut self, ..)` methods to configure them.
///
///```
/// # use failure::*;
/// # use serde_json::{json, Value};
/// # use proxmox_api::{*, schema::*};
/// #
/// const API_METHOD_HELLO: ApiMethod = ApiMethod::new(
///    &ApiHandler::Sync(&|_, _, _| {
///         Ok(json!("Hello world!"))
///    }),
///    &ObjectSchema::new("Hello World Example", &[])
/// );
/// const ROUTER: Router = Router::new()
///    .get(&API_METHOD_HELLO);
///```
pub struct Router {
    /// GET requests
    pub get: Option<&'static ApiMethod>,
    /// PUT requests
    pub put: Option<&'static ApiMethod>,
    /// POST requests
    pub post: Option<&'static ApiMethod>,
    /// DELETE requests
    pub delete: Option<&'static ApiMethod>,
    /// Used to find the correct API endpoint.
    pub subroute: Option<SubRoute>,
}

impl Router {
    /// Create a new Router.
    pub const fn new() -> Self {
        Self {
            get: None,
            put: None,
            post: None,
            delete: None,
            subroute: None,
        }
    }

    /// Configure a static map as `subroute`.
    pub const fn subdirs(mut self, map: SubdirMap) -> Self {
        self.subroute = Some(SubRoute::Map(map));
        self
    }

    /// Configure a `SubRoute::MatchAll` as `subroute`.
    pub const fn match_all(mut self, param_name: &'static str, router: &'static Router) -> Self {
        self.subroute = Some(SubRoute::MatchAll { router, param_name });
        self
    }

    /// Configure the GET method.
    pub const fn get(mut self, m: &'static ApiMethod) -> Self {
        self.get = Some(m);
        self
    }

    /// Configure the PUT method.
    pub const fn put(mut self, m: &'static ApiMethod) -> Self {
        self.put = Some(m);
        self
    }

    /// Configure the POST method.
    pub const fn post(mut self, m: &'static ApiMethod) -> Self {
        self.post = Some(m);
        self
    }

    /// Same as `post`, but expects an `AsyncHttp` handler.
    pub const fn upload(mut self, m: &'static ApiMethod) -> Self {
        // fixme: expect AsyncHttp
        self.post = Some(m);
        self
    }

    /// Same as `get`, but expects an `AsyncHttp` handler.
    pub const fn download(mut self, m: &'static ApiMethod) -> Self {
        // fixme: expect AsyncHttp
        self.get = Some(m);
        self
    }

    /// Same as `get`, but expects an `AsyncHttp` handler.
    pub const fn upgrade(mut self, m: &'static ApiMethod) -> Self {
        // fixme: expect AsyncHttp
        self.get = Some(m);
        self
    }

    /// Configure the DELETE method
    pub const fn delete(mut self, m: &'static ApiMethod) -> Self {
        self.delete = Some(m);
        self
    }

    /// Find the router for a specic path.
    ///
    /// - `components`: Path, split into individual components.
    /// - `uri_param`: Mutable hash map to store paramater from `MatchAll` router.
    pub fn find_route(
        &self,
        components: &[&str],
        uri_param: &mut HashMap<String, String>,
    ) -> Option<&Router> {
        if components.is_empty() {
            return Some(self);
        };

        let (dir, rest) = (components[0], &components[1..]);

        match self.subroute {
            None => {}
            Some(SubRoute::Map(dirmap)) => {
                if let Ok(ind) = dirmap.binary_search_by_key(&dir, |(name, _)| name) {
                    let (_name, router) = dirmap[ind];
                    //println!("FOUND SUBDIR {}", dir);
                    return router.find_route(rest, uri_param);
                }
            }
            Some(SubRoute::MatchAll { router, param_name }) => {
                //println!("URI PARAM {} = {}", param_name, dir); // fixme: store somewhere
                uri_param.insert(param_name.to_owned(), dir.into());
                return router.find_route(rest, uri_param);
            }
        }

        None
    }

    /// Lookup the API method for a specific path.
    /// - `components`: Path, split into individual components.
    /// - `method`: The HTTP method.
    /// - `uri_param`: Mutable hash map to store paramater from `MatchAll` router.
    pub fn find_method(
        &self,
        components: &[&str],
        method: Method,
        uri_param: &mut HashMap<String, String>,
    ) -> Option<&ApiMethod> {
        if let Some(info) = self.find_route(components, uri_param) {
            return match method {
                Method::GET => info.get,
                Method::PUT => info.put,
                Method::POST => info.post,
                Method::DELETE => info.delete,
                _ => None,
            };
        }
        None
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}
