#![feature(async_await)]

use std::pin::Pin;

use proxmox_api::Router;

#[test]
fn basic() {
    let info: &proxmox_api::ApiMethod = &methods::GET_PEOPLE;
    let get_subpath: &proxmox_api::ApiMethod = &methods::GET_SUBPATH;
    let router = Router::new()
        .subdir(
            "people",
            Router::new().parameter_subdir("person", Router::new().get(info)),
        )
        .subdir(
            "wildcard",
            Router::new().wildcard("subpath").get(get_subpath),
        );

    check_with_matched_params(&router, "people/foo", "person", "foo", "foo");
    check_with_matched_params(&router, "people//foo", "person", "foo", "foo");
    check_with_matched_params(&router, "wildcard", "subpath", "", "");
    check_with_matched_params(&router, "wildcard/", "subpath", "", "");
    check_with_matched_params(&router, "wildcard//", "subpath", "", "");
    check_with_matched_params(&router, "wildcard/dir1", "subpath", "dir1", "dir1");
    check_with_matched_params(
        &router,
        "wildcard/dir1/dir2",
        "subpath",
        "dir1/dir2",
        "dir1/dir2",
    );
    check_with_matched_params(&router, "wildcard/dir1//2", "subpath", "dir1//2", "dir1//2");
}

fn check_with_matched_params(
    router: &Router,
    path: &str,
    param_name: &str,
    param_value: &str,
    expected_body: &str,
) {
    let (target, params) = router
        .lookup(path)
        .expect(&format!("must be able to lookup '{}'", path));

    let params = params.expect(&format!(
        "expected parameters to be matched into '{}'",
        param_name,
    ));

    let apifn = target
        .get
        .as_ref()
        .expect(&format!("expected GET method on {}", path))
        .handler();

    let arg = params[param_name].as_str().expect(&format!(
        "expected lookup() to fill the '{}' parameter",
        param_name
    ));

    assert_eq!(
        arg, param_value,
        "lookup of '{}' should set '{}' to '{}'",
        path, param_name, param_value,
    );

    let response = futures::executor::block_on(Pin::from(apifn(params)))
        .expect("expected the simple test api function to be ready immediately");

    assert_eq!(response.status(), 200, "response status must be 200");

    let body =
        std::str::from_utf8(response.body().as_ref()).expect("expected a valid utf8 repsonse body");

    assert_eq!(
        body, expected_body,
        "response of {} should be '{}', got '{}'",
        path, expected_body, body,
    );
}

#[cfg(test)]
mod methods {
    use failure::{bail, Error};
    use http::Response;
    use lazy_static::lazy_static;
    use serde_derive::{Deserialize, Serialize};
    use serde_json::Value;

    use proxmox_api::{
        get_type_info, ApiFuture, ApiMethod, ApiOutput, ApiType, Parameter, TypeInfo,
    };

    pub async fn get_people(value: Value) -> ApiOutput {
        Ok(Response::builder()
            .status(200)
            .header("content-type", "application/json")
            .body(value["person"].as_str().unwrap().into())?)
    }

    pub async fn get_subpath(value: Value) -> ApiOutput {
        Ok(Response::builder()
            .status(200)
            .header("content-type", "application/json")
            .body(value["subpath"].as_str().unwrap().into())?)
    }

    lazy_static! {
        static ref GET_PEOPLE_PARAMS: Vec<Parameter> = {
            vec![Parameter {
                name: "person",
                description: "the person to get",
                type_info: get_type_info::<String>(),
            }]
        };
        pub static ref GET_PEOPLE: ApiMethod = {
            ApiMethod {
                description: "get some people",
                parameters: &GET_PEOPLE_PARAMS,
                return_type: get_type_info::<String>(),
                protected: false,
                reload_timezone: false,
                handler: |value: Value| -> ApiFuture { Box::pin(get_people(value)) },
            }
        };
        static ref GET_SUBPATH_PARAMS: Vec<Parameter> = {
            vec![Parameter {
                name: "subpath",
                description: "the matched relative subdir path",
                type_info: get_type_info::<String>(),
            }]
        };
        pub static ref GET_SUBPATH: ApiMethod = {
            ApiMethod {
                description: "get the 'subpath' parameter returned back",
                parameters: &GET_SUBPATH_PARAMS,
                return_type: get_type_info::<String>(),
                protected: false,
                reload_timezone: false,
                handler: |value: Value| -> ApiFuture { Box::pin(get_subpath(value)) },
            }
        };
    }

    #[derive(Deserialize, Serialize)]
    pub struct CubicMeters(f64);

    proxmox_api::unconstrained_api_type! {CubicMeters}

    #[derive(Deserialize, Serialize)]
    pub struct Thing {
        shape: String,
        size: CubicMeters,
    }

    impl ApiType for Thing {
        fn type_info() -> &'static TypeInfo {
            const INFO: TypeInfo = TypeInfo {
                name: "Thing",
                description: "A thing",
                complete_fn: None,
            };
            &INFO
        }

        fn verify(&self) -> Result<(), Error> {
            if self.shape == "flat" {
                bail!("flat shapes not allowed...");
            }
            Ok(())
        }
    }
}
