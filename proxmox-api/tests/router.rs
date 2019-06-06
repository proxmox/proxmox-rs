#![feature(async_await)]

use std::pin::Pin;

use proxmox_api::Router;

#[test]
fn basic() {
    let info: &proxmox_api::ApiMethod = &methods::GET_PEOPLE;
    let router = Router::new().subdir(
        "people",
        Router::new().parameter_subdir("person", Router::new().get(info)),
    );

    let (target, params) = router
        .lookup("people/foo")
        .expect("must be able to lookup 'people/foo'");

    let params = params.expect("expected people/foo to create a parameter object");

    let apifn = target
        .get
        .as_ref()
        .expect("expected GET method on people/foo")
        .handler();

    let person = params["person"]
        .as_str()
        .expect("expected lookup() to fill the 'person' parameter");

    assert!(
        person == "foo",
        "lookup of 'people/foo' should set 'person' to 'foo'"
    );

    let response = futures::executor::block_on(Pin::from(apifn(params)))
        .expect("expected the simple test api function to be ready immediately");

    assert!(response.status() == 200, "response status must be 200");

    let body =
        std::str::from_utf8(response.body().as_ref()).expect("expected a valid utf8 repsonse body");

    assert!(
        body == "foo",
        "repsonse of people/foo should simply be 'foo'"
    );
}

#[cfg(test)]
mod methods {
    use failure::{bail, Error};
    use http::Response;
    use lazy_static::lazy_static;
    use serde_derive::{Deserialize, Serialize};
    use serde_json::Value;

    use proxmox_api::{get_type_info, ApiFuture, ApiMethod, ApiOutput, ApiType, Parameter, TypeInfo};

    pub async fn get_people(value: Value) -> ApiOutput {
        Ok(Response::builder()
            .status(200)
            .header("content-type", "application/json")
            .body(value["person"].as_str().unwrap().into())?)
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
                handler: |value: Value| -> ApiFuture {
                    Box::pin(get_people(value))
                },
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
