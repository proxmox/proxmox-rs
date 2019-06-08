//! Proxmox API module. This provides utilities for HTTP and command line APIs.
//!
//! The main component here is the [`Router`] which is filled with entries pointing to
//! [`ApiMethodInfos`](crate::ApiMethodInfo).
//!
//! Note that you'll rarely need the [`Router`] type itself, as you'll most likely be creating them
//! with the `router` macro provided by the `proxmox-api-macro` crate.

use std::cell::Cell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Once;

use bytes::Bytes;
use failure::Error;
use http::Response;
use serde_json::Value;

mod api_output;
pub use api_output::*;

mod api_type;
pub use api_type::*;

/// Return type of an API method.
pub type ApiOutput = Result<Response<Bytes>, Error>;

/// Future type of an API method. In order to support `async fn` this is a pinned box.
pub type ApiFuture = Pin<Box<dyn Future<Output = ApiOutput>>>;

/// This enum specifies what to do when a subdirectory is requested from the current router.
///
/// For plain subdirectories a `Directories` entry is used.
///
/// When subdirectories are supposed to be passed as a `String` parameter to methods beneath the
/// current directory, a `Parameter` entry is used. Note that the parameter name is fixed at this
/// point, so all method calls beneath will receive a parameter ot that particular name.
pub enum SubRoute {
    /// This is used for plain subdirectories.
    Directories(HashMap<&'static str, Router>),

    /// Match subdirectories as the given parameter name to the underlying router.
    Parameter(&'static str, Box<Router>),
}

/// A router is a nested structure. On the one hand it contains HTTP method entries (`GET`, `PUT`,
/// ...), and on the other hand it contains sub directories. In some cases we want to match those
/// sub directories as parameters, so the nesting uses a `SubRoute` `enum` representing which of
/// the two is the case.
#[derive(Default)]
pub struct Router {
    /// The `GET` http method.
    pub get: Option<&'static dyn ApiMethodInfo>,

    /// The `PUT` http method.
    pub put: Option<&'static dyn ApiMethodInfo>,

    /// The `POST` http method.
    pub post: Option<&'static dyn ApiMethodInfo>,

    /// The `DELETE` http method.
    pub delete: Option<&'static dyn ApiMethodInfo>,

    /// Specifies the behavior of sub directories. See [`SubRoute`].
    pub subroute: Option<SubRoute>,
}

impl Router {
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

        let mut this = self;
        for component in path.split('/') {
            if component.is_empty() {
                // `foo//bar` or the first `/` in `/foo`
                continue;
            }
            this = match &this.subroute {
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

        Some((this, matched_params.map(Value::Object)))
    }

    /// Builder method to provide a `GET` method info.
    pub fn get<I>(mut self, method: &'static I) -> Self
    where
        I: ApiMethodInfo,
    {
        self.get = Some(method);
        self
    }

    /// Builder method to provide a `PUT` method info.
    pub fn put<I>(mut self, method: &'static I) -> Self
    where
        I: ApiMethodInfo,
    {
        self.put = Some(method);
        self
    }

    /// Builder method to provide a `POST` method info.
    pub fn post<I>(mut self, method: &'static I) -> Self
    where
        I: ApiMethodInfo,
    {
        self.post = Some(method);
        self
    }

    /// Builder method to provide a `DELETE` method info.
    pub fn delete<I>(mut self, method: &'static I) -> Self
    where
        I: ApiMethodInfo,
    {
        self.delete = Some(method);
        self
    }

    /// Builder method to make this router match the next subdirectory into a parameter.
    ///
    /// This is supposed to be used statically (via `lazy_static!), therefore we panic if we
    /// already have a subdir entry!
    pub fn parameter_subdir(mut self, parameter_name: &'static str, router: Router) -> Self {
        if self.subroute.is_some() {
            panic!("match_parameter can only be used once and without sub directories");
        }
        self.subroute = Some(SubRoute::Parameter(parameter_name, Box::new(router)));
        self
    }

    /// Builder method to add a regular directory entro to this router.
    ///
    /// This is supposed to be used statically (via `lazy_static!), therefore we panic if we
    /// already have a subdir entry!
    pub fn subdir(mut self, dir_name: &'static str, router: Router) -> Self {
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
}

/// We're supposed to only use types in the API which implement `ApiType`, which forces types ot
/// have a `verify` method. The idea is that all parameters used in the API are documented
/// somewhere with their formats and limits, which are checked when entering and leaving API entry
/// points.
///
/// Any API type is also required to implement `Serialize` and `DeserializeOwned`, since they're
/// read out of json `Value` types.
///
/// While this is very useful for structural types, we sometimes to want to be able to pass a
/// simple unconstrainted type like a `String` with no restrictions, so most basic types implement
/// `ApiType` as well.
//
// FIXME: I've actually moved most of this into the types in `api_type.rs` now, so this is
// probably unused at this point?
// `verify` should be moved to `TypeInfo` (for the type related verifier), and `Parameter` should
// get an additional verify method for constraints added by *methods*.
//
// We actually have 2 layers of validation:
//   When entering the API: The type validation
//       obviously a `String` should also be a string in the json object...
//       This does not happen when we call the method from rust-code as we have no json layer
//       there.
//   When entering the function: The input validation
//       if the function says `Integer`, the type itself has no validation other than that it has
//       to be an integer type, but the function may still say `minimum: 5, maximum: 10`.
//       This should also happen for direct calls from within rust, the `#[api]` macro can take
//       care of this.
//   When leaving the function: The output validation
//       Yep, we need to add this ;-)
pub trait ApiType {
    /// API types need to provide a `TypeInfo`, providing details about the underlying type.
    fn type_info() -> &'static TypeInfo;

    /// Additionally, ApiTypes must provide a way to verify their constraints!
    fn verify(&self) -> Result<(), Error>;

    /// This is a workaround for when we cannot name the type but have an object available we can
    /// call a method on. (We cannot call associated methods on objects without being able to write
    /// out the type, and rust has some restrictions as to what types are available.)
    // eg. nested generics:
    //     fn foo<T>() {
    //         fn bar<U>(x: &T) {
    //             cannot use T::method() here, but can use x.method()
    //             (compile error "can't use generic parameter of outer function",
    //             and yes, that's a stupid restriction as it is still completely static...)
    //         }
    //     }
    fn get_type_info(&self) -> &'static TypeInfo {
        Self::type_info()
    }
}

/// Option types are supposed to wrap their underlying types with an `optional:` text in their
/// description.
// BUT it requires some anti-static magic. And while this looks like the result of lazy_static!,
// it's not exactly the same, lazy_static! here does not actually work as it'll curiously produce
// the same error as we pointed out above in the `get_type_info` method (as it does a lot more
// extra stuff we don't need)...
impl<T: ApiType> ApiType for Option<T> {
    fn verify(&self) -> Result<(), Error> {
        if let Some(inner) = self {
            inner.verify()?
        }
        Ok(())
    }

    fn type_info() -> &'static TypeInfo {
        struct Data {
            info: Cell<Option<TypeInfo>>,
            once: Once,
            name: Cell<Option<String>>,
            description: Cell<Option<String>>,
        }
        unsafe impl Sync for Data {}
        static DATA: Data = Data {
            info: Cell::new(None),
            once: Once::new(),
            name: Cell::new(None),
            description: Cell::new(None),
        };
        DATA.once.call_once(|| {
            let info = T::type_info();
            DATA.name.set(Some(format!("optional: {}", info.name)));
            DATA.info.set(Some(TypeInfo {
                name: unsafe { (*DATA.name.as_ptr()).as_ref().unwrap().as_str() },
                description: unsafe { (*DATA.description.as_ptr()).as_ref().unwrap().as_str() },
                complete_fn: None,
            }));
        });
        unsafe { (*DATA.info.as_ptr()).as_ref().unwrap() }
    }
}

/// Any `Result<T, Error>` of course gets the same info as `T`, since this only means that it can
/// fail...
impl<T: ApiType> ApiType for Result<T, Error> {
    fn verify(&self) -> Result<(), Error> {
        if let Ok(inner) = self {
            inner.verify()?
        }
        Ok(())
    }

    fn type_info() -> &'static TypeInfo {
        <T as ApiType>::type_info()
    }
}

/// This is not supposed to be used, but can be if needed. This will provide an empty `ApiType`
/// declaration with no description and no verifier.
///
/// This rarely makes sense, but sometimes a `string` is just a `string`.
#[macro_export]
macro_rules! unconstrained_api_type {
    ($type:ty $(, $more:ty)*) => {
        impl $crate::ApiType for $type {
            fn verify(&self) -> Result<(), ::failure::Error> {
                Ok(())
            }

            fn type_info() -> &'static $crate::TypeInfo {
                const INFO: $crate::TypeInfo = $crate::TypeInfo {
                    name: stringify!($type),
                    description: stringify!($type),
                    complete_fn: None,
                };
                &INFO
            }
        }

        $crate::unconstrained_api_type!{$($more),*}
    };
    () => {};
}

unconstrained_api_type! {Value} // basically our API's "any" type
unconstrained_api_type! {&str}
unconstrained_api_type! {String, isize, usize, i64, u64, i32, u32, i16, u16, i8, u8, f64, f32}
unconstrained_api_type! {Vec<String>}

// Raw return types are also okay:
unconstrained_api_type! {Response<Bytes>}

// FIXME: make const once feature(const_fn) is stable!
pub fn get_type_info<T: ApiType>() -> &'static TypeInfo {
    T::type_info()
}
