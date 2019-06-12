//! This contains traits used to implement methods to be added to the `Router`.

use std::cell::Cell;
use std::sync::Once;

use failure::Error;
use http::Response;
use serde_json::Value;

/// Method entries in a `Router` are actually just `&dyn ApiMethodInfo` trait objects.
/// This contains all the info required to call, document, or command-line-complete parameters for
/// a method.
pub trait ApiMethodInfo<Body> {
    fn description(&self) -> &'static str;
    fn parameters(&self) -> &'static [Parameter];
    fn return_type(&self) -> &'static TypeInfo;
    fn protected(&self) -> bool;
    fn reload_timezone(&self) -> bool;
    fn handler(&self) -> fn(Value) -> super::ApiFuture<Body>;
}

/// Shortcut to not having to type it out. This function signature is just a dummy and not yet
/// stabalized!
pub type CompleteFn = fn(&str) -> Vec<String>;

/// Provides information about a method's parameter. Every parameter has a name and must be
/// documented with a description, type information, and optional constraints.
pub struct Parameter {
    pub name: &'static str,
    pub description: &'static str,
    pub type_info: fn() -> &'static TypeInfo,
}

/// Bare type info. Types themselves should also have a description, even if a method's parameter
/// usually overrides it. Ideally we can hyperlink the parameter to the type information in the
/// generated documentation.
pub struct TypeInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub complete_fn: Option<CompleteFn>,
}

/// Until we can slap `#[api]` onto all the functions we can start translating our existing
/// `ApiMethod` structs to this new layout.
/// Otherwise this is mostly there so we can run the tests in the tests subdirectory without
/// depending on the api-macro crate. Tests using the macros belong into the api-macro crate itself
/// after all!
pub struct ApiMethod<Body> {
    pub description: &'static str,
    pub parameters: &'static [Parameter],
    pub return_type: &'static TypeInfo,
    pub protected: bool,
    pub reload_timezone: bool,
    pub handler: fn(Value) -> super::ApiFuture<Body>,
}

impl<Body> ApiMethodInfo<Body> for ApiMethod<Body> {
    fn description(&self) -> &'static str {
        self.description
    }

    fn parameters(&self) -> &'static [Parameter] {
        self.parameters
    }

    fn return_type(&self) -> &'static TypeInfo {
        self.return_type
    }

    fn protected(&self) -> bool {
        self.protected
    }

    fn reload_timezone(&self) -> bool {
        self.reload_timezone
    }

    fn handler(&self) -> fn(Value) -> super::ApiFuture<Body> {
        self.handler
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
impl<Body> ApiType for Response<Body> {
    fn verify(&self) -> Result<(), Error> {
        Ok(())
    }

    fn type_info() -> &'static TypeInfo {
        const INFO: TypeInfo = TypeInfo {
            name: "http::Response<>",
            description: "A raw http response",
            complete_fn: None,
        };
        &INFO
    }
}

// FIXME: make const once feature(const_fn) is stable!
pub fn get_type_info<T: ApiType>() -> &'static TypeInfo {
    T::type_info()
}
