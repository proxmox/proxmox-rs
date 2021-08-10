use proxmox::api::api;
use proxmox::api::schema::Updater;

#[api]
/// An example of a simple struct type.
#[derive(Updater)]
#[serde(rename_all = "kebab-case")]
pub struct Simple {
    /// A test string.
    one_field: String,

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
    /// An extra field not part of the flattened struct.
    extra: String,

    #[serde(skip_serializing_if = "Updater::is_empty")]
    simple: Option<Simple>,

    /// A field which should not appear in the updater.
    #[updater(skip)]
    not_in_updater: String,
}
