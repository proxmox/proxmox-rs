#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use proxmox_api_macro::api;
use proxmox_schema::{ApiType, Updater, UpdaterType};

// Helpers for type checks:
struct AssertTypeEq<T>(T);
macro_rules! assert_type_eq {
    ($what:ident, $a:ty, $b:ty) => {
        #[allow(dead_code, unreachable_patterns)]
        fn $what(have: AssertTypeEq<$a>) {
            match have {
                AssertTypeEq::<$b>(_) => (),
            }
        }
    };
}

#[api(min_length: 3, max_length: 64)]
#[derive(proxmox_api_macro::UpdaterType)]
/// Custom String.
pub struct Custom(String);
assert_type_eq!(
    custom_type,
    <Custom as UpdaterType>::Updater,
    Option<Custom>
);

#[api]
/// An example of a simple struct type.
#[derive(Updater)]
#[serde(rename_all = "kebab-case")]
pub struct Simple {
    /// A test string.
    one_field: String,

    /// Another test value.
    #[serde(skip_serializing_if = "Option::is_empty")]
    opt: Option<String>,
}

#[test]
fn test_simple() {
    pub const TEST_SCHEMA: ::proxmox_schema::Schema = ::proxmox_schema::ObjectSchema::new(
        "An example of a simple struct type.",
        &[
            (
                "one-field",
                true,
                &::proxmox_schema::StringSchema::new("A test string.").schema(),
            ),
            (
                "opt",
                true,
                &::proxmox_schema::StringSchema::new("Another test value.").schema(),
            ),
        ],
    )
    .schema();

    assert_eq!(TEST_SCHEMA, SimpleUpdater::API_SCHEMA);
}

#[api(
    properties: {
        simple: { type: Simple },
    },
)]
/// A second struct so we can test flattening.
#[derive(Updater)]
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
#[derive(Updater)]
#[serde(rename_all = "kebab-case")]
pub struct SuperComplex {
    /// An extra field.
    extra: String,

    simple: Simple,

    /// A field which should not appear in the updater.
    #[updater(skip)]
    not_in_updater: String,

    /// A custom type with an Updatable implementation.
    custom: Custom,
}
#[test]
fn test_super_complex() {
    pub const TEST_SCHEMA: ::proxmox_schema::Schema = ::proxmox_schema::ObjectSchema::new(
        "One of the baaaad cases.",
        &[
            ("custom", true, &<Option<Custom> as ApiType>::API_SCHEMA),
            (
                "extra",
                true,
                &::proxmox_schema::StringSchema::new("An extra field.").schema(),
            ),
            (
                "simple",
                true,
                //&<<Simple as UpdaterType>::Updater as ApiType>::API_SCHEMA,
                &SimpleUpdater::API_SCHEMA,
            ),
        ],
    )
    .schema();

    assert_eq!(TEST_SCHEMA, SuperComplexUpdater::API_SCHEMA);
}

#[api]
/// A simple string wrapper.
#[derive(Default, Deserialize, Serialize, Updater)]
struct MyType(String);

impl proxmox_schema::UpdaterType for MyType {
    type Updater = Option<Self>;
}

impl MyType {
    fn should_skip(&self) -> bool {
        self.0 == "skipme"
    }
}

#[api(
    properties: {
        more: { type: MyType },
    },
)]
/// A struct where we replace serde attributes.
#[derive(Deserialize, Serialize, Updater)]
pub struct WithSerde {
    /// Simple string.
    data: String,

    #[serde(skip_serializing_if = "MyType::should_skip", default)]
    #[updater(serde(skip_serializing_if = "Option::is_none"))]
    more: MyType,
}

#[api(
    properties: {
        another: { type: MyType },
    },
)]
/// A struct where we replace serde attributes.
#[derive(Deserialize, Serialize, Updater)]
pub struct RenamedStuff {
    /// Simple string.
    data: String,

    #[serde(rename = "another")]
    #[updater(skip)]
    more: MyType,
}

#[api]
/// A struct with conditionally compiled fields.
#[derive(Updater)]
#[serde(rename_all = "kebab-case")]
pub struct WithCfg {
    /// A test string.
    one_field: String,

    /// Enabled field.
    #[cfg(true)]
    enabled_field: String,

    /// Disabled field.
    #[cfg(false)]
    disabled_field: String,
}

#[test]
fn test_with_cfg() {
    pub const TEST_SCHEMA: ::proxmox_schema::Schema = ::proxmox_schema::ObjectSchema::new(
        "A struct with conditionally compiled fields.",
        &[
            (
                "enabled-field",
                true,
                &::proxmox_schema::StringSchema::new("Enabled field.").schema(),
            ),
            (
                "one-field",
                true,
                &::proxmox_schema::StringSchema::new("A test string.").schema(),
            ),
        ],
    )
    .schema();

    assert_eq!(TEST_SCHEMA, WithCfgUpdater::API_SCHEMA);

    let mut updater = WithCfgUpdater::default();
    assert!(updater.is_empty());
    updater.enabled_field = Some("value".to_string());
    assert!(!updater.is_empty());
}

#[api]
/// A struct without any compiled-in updater field.
#[derive(Updater)]
pub struct AllCfg {
    /// Not part of the updater.
    #[updater(skip)]
    name: String,

    /// Disabled field.
    #[cfg(false)]
    disabled_field: String,
}

#[test]
fn test_all_cfg() {
    pub const TEST_SCHEMA: ::proxmox_schema::Schema =
        ::proxmox_schema::ObjectSchema::new("A struct without any compiled-in updater field.", &[])
            .schema();

    assert_eq!(TEST_SCHEMA, AllCfgUpdater::API_SCHEMA);
    assert!(AllCfgUpdater::default().is_empty());
}

#[api(
    properties: {
        simple: { type: Simple },
    },
)]
/// A struct with a configured-out flattened field.
#[derive(Updater)]
pub struct FlattenedWithCfg {
    /// An extra field.
    extra: String,

    #[cfg(false)]
    #[serde(flatten)]
    simple: Simple,
}

#[test]
fn test_flattened_with_cfg() {
    match FlattenedWithCfgUpdater::API_SCHEMA {
        ::proxmox_schema::Schema::AllOf(all_of) => assert_eq!(all_of.list.len(), 1),
        _ => panic!("expected an AllOf schema"),
    }

    let mut updater = FlattenedWithCfgUpdater::default();
    assert!(updater.is_empty());
    updater.extra = Some("value".to_string());
    assert!(!updater.is_empty());
}

#[api]
/// A struct whose fields are all skipped for the updater.
#[derive(Updater)]
pub struct AllSkipped {
    /// Not part of the updater.
    #[updater(skip)]
    name: String,
}

#[test]
fn test_all_skipped() {
    assert!(AllSkippedUpdater::default().is_empty());
}

#[api]
/// A struct with renamed fields.
#[derive(Deserialize, Serialize, Updater)]
pub struct Renamed {
    /// A field whose name is not a valid identifier.
    #[serde(rename = "wire-name")]
    value: String,

    /// A renamed field which should not appear in the updater.
    #[serde(rename = "skipped-name")]
    #[updater(skip)]
    skipped: String,
}

#[test]
fn test_renamed() {
    pub const TEST_SCHEMA: ::proxmox_schema::Schema = ::proxmox_schema::ObjectSchema::new(
        "A struct with renamed fields.",
        &[(
            "wire-name",
            true,
            &::proxmox_schema::StringSchema::new("A field whose name is not a valid identifier.")
                .schema(),
        )],
    )
    .schema();

    assert_eq!(TEST_SCHEMA, RenamedUpdater::API_SCHEMA);

    let updater = RenamedUpdater {
        value: Some("value".to_string()),
    };
    assert!(!updater.is_empty());
    assert_eq!(
        serde_json::to_value(&updater).unwrap(),
        serde_json::json!({ "wire-name": "value" }),
    );
}

#[api(
    properties: {
        "inner-part": { type: Simple },
    },
)]
/// A struct with a flattened field whose name is not a valid identifier.
#[derive(Updater)]
#[serde(rename_all = "kebab-case")]
pub struct FlattenedRenamed {
    /// An extra field.
    extra_field: String,

    #[serde(flatten)]
    inner_part: Simple,
}

#[test]
fn test_flattened_renamed() {
    match FlattenedRenamedUpdater::API_SCHEMA {
        ::proxmox_schema::Schema::AllOf(all_of) => assert_eq!(all_of.list.len(), 2),
        _ => panic!("expected an AllOf schema"),
    }
}
