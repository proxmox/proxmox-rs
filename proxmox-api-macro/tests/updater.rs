#[cfg(not(feature = "noserde"))]
use serde::{Deserialize, Serialize};
use serde_json::Value;

use proxmox::api::api;
use proxmox::api::schema::{Updatable, Updater};

#[api]
/// An example of a simple struct type.
#[cfg_attr(not(feature = "noserde"), derive(Deserialize, Serialize))]
#[derive(Debug, PartialEq, Updater)]
pub struct Simple {
    /// A test string.
    one: String,

    /// An optional auto-derived value for testing:
    #[serde(skip_serializing_if = "Option::is_empty")]
    opt: Option<String>,
}

#[api(
    properties: {
        simple: { type: Simple },
    },
)]
/// A second struct so we can test flattening.
#[cfg_attr(not(feature = "noserde"), derive(Deserialize, Serialize))]
#[derive(Debug, PartialEq, Updater)]
pub struct Complex {
    /// An extra field not part of the flattened struct.
    extra: String,

    #[serde(flatten)]
    simple: Simple,
}

#[api(
    properties: {
        simple: {
            type: Simple,
            optional: true,
        },
    },
)]
/// One of the baaaad cases.
#[cfg_attr(not(feature = "noserde"), derive(Deserialize, Serialize))]
#[derive(Debug, PartialEq, Updater)]
pub struct SuperComplex {
    /// An extra field not part of the flattened struct.
    extra: String,

    #[serde(skip_serializing_if = "Updater::is_empty")]
    simple: Option<Simple>,
}

#[api(
    properties: {
            complex: { type: Complex },
    },
)]
/// Something with "fixed" values we cannot update but require for creation.
#[cfg_attr(not(feature = "noserde"), derive(Deserialize, Serialize))]
#[derive(Debug, PartialEq, Updater)]
pub struct Creatable {
    /// An ID which cannot be changed later.
    #[updater(fixed)]
    id: String,

    /// Some parameter we're allowed to change with an updater.
    name: String,

    /// Optional additional information.
    #[serde(skip_serializing_if = "Updater::is_empty", default)]
    info: Option<String>,

    /// Optional additional information 2.
    #[serde(skip_serializing_if = "Updater::is_empty", default)]
    info2: Option<String>,

    /// Super complex additional data
    #[serde(flatten)]
    complex: Complex,
}

struct RpcEnv;
impl proxmox::api::RpcEnvironment for RpcEnv {
    fn result_attrib_mut(&mut self) -> &mut Value {
        panic!("result_attrib_mut called");
    }

    fn result_attrib(&self) -> &Value {
        panic!("result_attrib called");
    }

    /// The environment type
    fn env_type(&self) -> proxmox::api::RpcEnvironmentType {
        panic!("env_type called");
    }

    /// Set authentication id
    fn set_auth_id(&mut self, user: Option<String>) {
        let _ = user;
        panic!("set_auth_id called");
    }

    /// Get authentication id
    fn get_auth_id(&self) -> Option<String> {
        panic!("get_auth_id called");
    }
}

mod test_creatable {
    use anyhow::{bail, Error};
    use serde_json::json;

    use proxmox::api::schema::Updatable;
    use proxmox_api_macro::api;

    use super::*;

    static mut TEST_OBJECT: Option<Creatable> = None;

    #[api(
        input: {
            properties: {
                thing: { flatten: true, type: CreatableUpdater },
            },
        },
    )]
    /// Test method to create an object.
    ///
    /// Returns: the object's ID.
    pub fn create_thing(thing: CreatableUpdater) -> Result<String, Error> {
        if unsafe { TEST_OBJECT.is_some() } {
            bail!("object exists");
        }

        let obj = Creatable::try_build_from(thing)?;
        let id = obj.id.clone();

        unsafe {
            TEST_OBJECT = Some(obj);
        }

        Ok(id)
    }

    #[api(
        input: {
            properties: {
                thing: { flatten: true, type: CreatableUpdater },
                delete: {
                    optional: true,
                    description: "list of properties to delete",
                    type: Array,
                    items: {
                        description: "field name to delete",
                        type: String,
                    },
                },
            },
        },
    )]
    /// Test method to update an object.
    pub fn update_thing(thing: CreatableUpdater, delete: Option<Vec<String>>) -> Result<(), Error> {
        let delete = delete.unwrap_or_default();
        match unsafe { &mut TEST_OBJECT } {
            Some(obj) => obj.update_from(thing, &delete),
            None => bail!("object has not been created yet"),
        }
    }

    #[test]
    fn test() {
        let _ = api_function_create_thing(
            json!({ "name": "The Name" }),
            &API_METHOD_CREATE_THING,
            &mut RpcEnv,
        )
        .expect_err("create_thing should fail without an ID");

        let _ = api_function_create_thing(
            json!({ "id": "Id1" }),
            &API_METHOD_CREATE_THING,
            &mut RpcEnv,
        )
        .expect_err("create_thing should fail without a name");

        let value = api_function_create_thing(
            json!({
                "id": "Id1",
                "name": "The Name",
                "extra": "Extra Info",
                "one": "Part of Simple",
                "info2": "More Info 2",
            }),
            &API_METHOD_CREATE_THING,
            &mut RpcEnv,
        )
        .expect("create_thing should work");
        assert_eq!(value, "Id1");
        assert_eq!(
            unsafe { &TEST_OBJECT },
            &Some(Creatable {
                id: "Id1".to_string(),
                name: "The Name".to_string(),
                info: None,
                info2: Some("More Info 2".to_string()),
                complex: Complex {
                    extra: "Extra Info".to_string(),
                    simple: Simple {
                        one: "Part of Simple".to_string(),
                        opt: None,
                    },
                },
            }),
        );

        let _ = api_function_update_thing(
            json!({
                "id": "Poop",
            }),
            &API_METHOD_UPDATE_THING,
            &mut RpcEnv,
        )
        .expect_err("shouldn't be allowed to update the ID");

        let _ = api_function_update_thing(
            json!({
                "info": "Updated Info",
                "delete": ["info2"],
            }),
            &API_METHOD_UPDATE_THING,
            &mut RpcEnv,
        )
        .expect("should be allowed to update the optional field");

        assert_eq!(
            unsafe { &TEST_OBJECT },
            &Some(Creatable {
                id: "Id1".to_string(),
                name: "The Name".to_string(),
                info: Some("Updated Info".to_string()),
                info2: None,
                complex: Complex {
                    extra: "Extra Info".to_string(),
                    simple: Simple {
                        one: "Part of Simple".to_string(),
                        opt: None,
                    },
                },
            }),
        );

        let _ = api_function_update_thing(
            json!({
                "extra": "Partial flatten update",
            }),
            &API_METHOD_UPDATE_THING,
            &mut RpcEnv,
        )
        .expect("should be allowed to update the parts of a flattened field");
        assert_eq!(
            unsafe { &TEST_OBJECT },
            &Some(Creatable {
                id: "Id1".to_string(),
                name: "The Name".to_string(),
                info: Some("Updated Info".to_string()),
                info2: None,
                complex: Complex {
                    extra: "Partial flatten update".to_string(),
                    simple: Simple {
                        one: "Part of Simple".to_string(),
                        opt: None,
                    },
                },
            }),
        );

        let _ = api_function_update_thing(
            json!({
                "opt": "Deeply nested optional update.",
            }),
            &API_METHOD_UPDATE_THING,
            &mut RpcEnv,
        )
        .expect("should be allowed to update the parts of a deeply nested struct");
        assert_eq!(
            unsafe { &TEST_OBJECT },
            &Some(Creatable {
                id: "Id1".to_string(),
                name: "The Name".to_string(),
                info: Some("Updated Info".to_string()),
                info2: None,
                complex: Complex {
                    extra: "Partial flatten update".to_string(),
                    simple: Simple {
                        one: "Part of Simple".to_string(),
                        opt: Some("Deeply nested optional update.".to_string()),
                    },
                },
            }),
        );

        let _ = api_function_update_thing(
            json!({
                "delete": ["opt"],
            }),
            &API_METHOD_UPDATE_THING,
            &mut RpcEnv,
        )
        .expect("should be allowed to remove parts of a deeply nested struct");
        assert_eq!(
            unsafe { &TEST_OBJECT },
            &Some(Creatable {
                id: "Id1".to_string(),
                name: "The Name".to_string(),
                info: Some("Updated Info".to_string()),
                info2: None,
                complex: Complex {
                    extra: "Partial flatten update".to_string(),
                    simple: Simple {
                        one: "Part of Simple".to_string(),
                        opt: None,
                    },
                },
            }),
        );
    }
}
