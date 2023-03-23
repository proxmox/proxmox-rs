use serde::{Deserialize, Serialize};

#[cfg(feature = "api-types")]
use proxmox_schema::api;

#[cfg_attr(feature = "api-types", api)]
/// A TFA entry type.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TfaType {
    /// A TOTP entry type.
    Totp,
    /// A U2F token entry.
    U2f,
    /// A Webauthn token entry.
    Webauthn,
    /// Recovery tokens.
    Recovery,
    /// Yubico authentication entry.
    Yubico,
}
serde_plain::derive_display_from_serialize!(TfaType);
serde_plain::derive_fromstr_from_deserialize!(TfaType);

#[cfg_attr(feature = "api-types", api)]
/// Over the API we only provide this part when querying a user's second factor list.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TfaInfo {
    /// The id used to reference this entry.
    pub id: String,

    /// User chosen description for this entry.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,

    /// Creation time of this entry as unix epoch.
    pub created: i64,

    /// Whether this TFA entry is currently enabled.
    #[serde(skip_serializing_if = "is_default_tfa_enable")]
    #[serde(default = "default_tfa_enable")]
    pub enable: bool,
}

const fn default_tfa_enable() -> bool {
    true
}

const fn is_default_tfa_enable(v: &bool) -> bool {
    *v
}

impl TfaInfo {
    /// For recovery keys we have a fixed entry.
    pub fn recovery(created: i64) -> Self {
        Self {
            id: "recovery".to_string(),
            description: String::new(),
            enable: true,
            created,
        }
    }
}

#[cfg_attr(
    feature = "api-types",
    api(
        properties: {
            type: { type: TfaType },
            info: { type: TfaInfo },
        },
    )
)]
/// A TFA entry for a user.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypedTfaInfo {
    #[serde(rename = "type")]
    pub ty: TfaType,

    #[serde(flatten)]
    pub info: TfaInfo,
}

#[cfg_attr(feature = "api-types", api(
    properties: {
        recovery: {
            description: "A list of recovery codes as integers.",
            type: Array,
            items: {
                type: Integer,
                description: "A one-time usable recovery code entry.",
            },
        },
    },
))]
/// The result returned when adding TFA entries to a user.
#[derive(Default, Deserialize, Serialize)]
pub struct TfaUpdateInfo {
    /// The id if a newly added TFA entry.
    pub id: Option<String>,

    /// When adding u2f entries, this contains a challenge the user must respond to in order to
    /// finish the registration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub challenge: Option<String>,

    /// When adding recovery codes, this contains the list of codes to be displayed to the user
    /// this one time.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub recovery: Vec<String>,
}

impl TfaUpdateInfo {
    pub(crate) fn with_id(id: String) -> Self {
        Self {
            id: Some(id),
            ..Default::default()
        }
    }
}
