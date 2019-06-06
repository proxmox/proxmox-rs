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

pub type ApiOutput = Result<Response<Bytes>, Error>;
pub type ApiFuture = Pin<Box<dyn Future<Output = ApiOutput>>>;
pub type ApiFn = Box<dyn Fn(Value) -> ApiFuture>;

pub enum SubRoute {
    Directories(HashMap<&'static str, Router>),
    Parameter(&'static str, Box<Router>),
}

#[derive(Default)]
pub struct Router {
    pub get: Option<&'static dyn ApiMethodInfo>,
    pub put: Option<&'static dyn ApiMethodInfo>,
    pub post: Option<&'static dyn ApiMethodInfo>,
    pub delete: Option<&'static dyn ApiMethodInfo>,
    pub subroute: Option<SubRoute>,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn lookup<T: AsRef<str>>(&self, path: T) -> Option<(&Self, Option<Value>)> {
        self.lookup_do(path.as_ref())
    }

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

    pub fn get<I>(mut self, method: &'static I) -> Self
    where
        I: ApiMethodInfo,
    {
        self.get = Some(method);
        self
    }

    pub fn put<I>(mut self, method: &'static I) -> Self
    where
        I: ApiMethodInfo,
    {
        self.put = Some(method);
        self
    }

    pub fn post<I>(mut self, method: &'static I) -> Self
    where
        I: ApiMethodInfo,
    {
        self.post = Some(method);
        self
    }

    pub fn delete<I>(mut self, method: &'static I) -> Self
    where
        I: ApiMethodInfo,
    {
        self.delete = Some(method);
        self
    }

    // To be used statically, therefore we panic otherwise!
    pub fn parameter_subdir(mut self, parameter_name: &'static str, router: Router) -> Self {
        if self.subroute.is_some() {
            panic!("match_parameter can only be used once and without sub directories");
        }
        self.subroute = Some(SubRoute::Parameter(parameter_name, Box::new(router)));
        self
    }

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
pub trait ApiType {
    fn type_info() -> &'static TypeInfo;
    fn verify(&self) -> Result<(), Error>;

    fn get_type_info(&self) -> &'static TypeInfo {
        Self::type_info()
    }
}

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

// FIXME: make const once feature(const_fn) is stable!
pub fn get_type_info<T: ApiType>() -> &'static TypeInfo {
    T::type_info()
}
