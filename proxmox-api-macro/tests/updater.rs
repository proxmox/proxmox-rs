#![allow(dead_code)]

use proxmox_schema::{api, ApiType, Updater, UpdaterType};

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
#[derive(UpdaterType)]
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
