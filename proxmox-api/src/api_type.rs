//! This contains traits used to implement methods to be added to the `Router`.

use failure::{bail, Error};
use http::Response;
use serde_json::{json, Value};

/// Method entries in a `Router` are actually just `&dyn ApiMethodInfo` trait objects.
/// This contains all the info required to call, document, or command-line-complete parameters for
/// a method.
pub trait ApiMethodInfo {
    fn description(&self) -> &'static str;
    fn parameters(&self) -> &'static [Parameter];
    fn return_type(&self) -> &'static TypeInfo;
    fn protected(&self) -> bool;
    fn reload_timezone(&self) -> bool;
}

pub trait ApiHandler: ApiMethodInfo + Send + Sync {
    type Body;

    fn call(&self, params: Value) -> super::ApiFuture<Self::Body>;
    fn method_info(&self) -> &(dyn ApiMethodInfo + Send + Sync);
}

impl<Body: 'static> dyn ApiHandler<Body = Body> {
    pub fn call_as<ToBody>(&self, params: Value) -> super::ApiFuture<ToBody>
    where
        Body: Into<ToBody>,
    {
        use futures::future::TryFutureExt;
        Box::pin(self.call(params).map_ok(|res| res.map(|res| res.into())))
    }
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

impl Parameter {
    pub fn api_dump(&self) -> (&'static str, Value) {
        (
            self.name,
            json!({
                "description": self.description,
                "type": (self.type_info)().name,
            }),
        )
    }

    /// Parse a commnd line option: if it is None, we only saw an `--option` without value, this is
    /// fine for booleans. If we saw a value, we should try to parse it out into a json value. For
    /// string parameters this means passing them as is, for others it means using FromStr...
    pub fn parse_cli(&self, name: &str, value: Option<&str>) -> Result<Value, Error> {
        let info = (self.type_info)();
        match info.parse_cli {
            Some(func) => func(name, value),
            None => bail!(
                "cannot parse parameter '{}' as command line parameter",
                name
            ),
        }
    }
}

/// Bare type info. Types themselves should also have a description, even if a method's parameter
/// usually overrides it. Ideally we can hyperlink the parameter to the type information in the
/// generated documentation.
pub struct TypeInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub complete_fn: Option<CompleteFn>,
    pub parse_cli: Option<fn(name: &str, value: Option<&str>) -> Result<Value, Error>>,
}

impl TypeInfo {
    pub fn api_dump(&self) -> Value {
        Value::String(self.name.to_string())
    }
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

impl<Body> ApiMethodInfo for ApiMethod<Body> {
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
}

impl<Body> ApiHandler for ApiMethod<Body> {
    type Body = Body;

    fn call(&self, params: Value) -> super::ApiFuture<Body> {
        (self.handler)(params)
    }

    fn method_info(&self) -> &(dyn ApiMethodInfo + Send + Sync) {
        self as _
    }
}

impl dyn ApiMethodInfo + Send + Sync {
    pub fn api_dump(&self) -> Value {
        let parameters = Value::Object(std::iter::FromIterator::from_iter(
            self.parameters()
                .iter()
                .map(|p| p.api_dump())
                .map(|(name, value)| (name.to_string(), value)),
        ));

        json!({
            "description": self.description(),
            "protected": self.protected(),
            "reload-timezone": self.reload_timezone(),
            "parameters": parameters,
            "returns": self.return_type().api_dump(),
        })
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
pub trait ApiType: Sized {
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

    #[inline]
    fn should_skip_serialization(&self) -> bool {
        false
    }

    #[inline]
    fn deserialization_check<F, E>(this: Option<Self>, missing_error: F) -> Result<Self, E>
    where
        F: FnOnce() -> E,
    {
        this.ok_or_else(missing_error)
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
        // FIXME: rust does not parameterize statics by the outer functions' generic parameters, so
        // we cannot build special TypeInfo objects for options...
        <T as ApiType>::type_info()
        /* DOES NOT WORK:
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
            DATA.description
                .set(Some(format!("optional: {}", info.description)));
            DATA.info.set(Some(TypeInfo {
                name: unsafe { (*DATA.name.as_ptr()).as_ref().unwrap().as_str() },
                description: unsafe { (*DATA.description.as_ptr()).as_ref().unwrap().as_str() },
                complete_fn: info.complete_fn,
                parse_cli: info.parse_cli,
            }));
        });
        unsafe { (*DATA.info.as_ptr()).as_ref().unwrap() }
        */
    }

    #[inline]
    fn should_skip_serialization(&self) -> bool {
        self.is_none()
    }

    #[inline]
    fn deserialization_check<F, E>(this: Option<Self>, _missing_error: F) -> Result<Self, E>
    where
        F: FnOnce() -> E,
    {
        Ok(this.unwrap_or(None))
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
/// This requires that the type already implements the `ParseCli` trait (or has a `parse_cli` type
/// of the same signature in view from any other trait).
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
                    parse_cli: Some(<$type as $crate::cli::ParseCli>::parse_cli),
                };
                &INFO
            }
        }

        $crate::unconstrained_api_type!{$($more),*}
    };
    () => {};
}

unconstrained_api_type! {Value} // basically our API's "any" type
unconstrained_api_type! {String, &str}
unconstrained_api_type! {bool}
unconstrained_api_type! {isize, usize, i64, u64, i32, u32, i16, u16, i8, u8, f64, f32}
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
            parse_cli: None,
        };
        &INFO
    }
}

// FIXME: make const once feature(const_fn) is stable!
pub fn get_type_info<T: ApiType>() -> &'static TypeInfo {
    T::type_info()
}

/// API methods can have different body types. For the CLI we don't care whether it is a
/// hyper::Body or a bytes::Bytes (also because we don't care for partia bodies etc.), so the
/// output needs to be wrapped to a common format. So basically the CLI will only ever see
/// `ApiOutput<Bytes>`.
pub trait UnifiedApiMethod<Body>: Send + Sync {
    fn parameters(&self) -> &'static [Parameter];
    fn call(&self, params: Value) -> super::ApiFuture<Body>;
}

impl<T: Send + Sync + 'static, Body> UnifiedApiMethod<Body> for T
where
    T: ApiHandler,
    T::Body: 'static + Into<Body>,
{
    fn parameters(&self) -> &'static [Parameter] {
        ApiMethodInfo::parameters(self)
    }

    fn call(&self, params: Value) -> super::ApiFuture<Body> {
        (self as &dyn ApiHandler<Body = T::Body>).call_as(params)
    }
}
