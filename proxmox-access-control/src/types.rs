use serde::{Deserialize, Serialize};

use const_format::concatcp;

use proxmox_auth_api::types::{Authid, Userid, PROXMOX_TOKEN_ID_SCHEMA};
use proxmox_schema::{
    api,
    api_types::{COMMENT_SCHEMA, SAFE_ID_REGEX_STR, SINGLE_LINE_COMMENT_FORMAT},
    const_regex, ApiStringFormat, BooleanSchema, IntegerSchema, Schema, StringSchema, Updater,
};

pub const ENABLE_USER_SCHEMA: Schema = BooleanSchema::new(
    "Enable the account (default). You can set this to '0' to disable the account.",
)
.default(true)
.schema();

pub const EXPIRE_USER_SCHEMA: Schema = IntegerSchema::new(
    "Account expiration date (seconds since epoch). '0' means no expiration date.",
)
.default(0)
.minimum(0)
.schema();

pub const FIRST_NAME_SCHEMA: Schema = StringSchema::new("First name.")
    .format(&SINGLE_LINE_COMMENT_FORMAT)
    .min_length(2)
    .max_length(64)
    .schema();

pub const LAST_NAME_SCHEMA: Schema = StringSchema::new("Last name.")
    .format(&SINGLE_LINE_COMMENT_FORMAT)
    .min_length(2)
    .max_length(64)
    .schema();

pub const EMAIL_SCHEMA: Schema = StringSchema::new("E-Mail Address.")
    .format(&SINGLE_LINE_COMMENT_FORMAT)
    .min_length(2)
    .max_length(64)
    .schema();

const_regex! {
    pub ACL_PATH_REGEX = concatcp!(r"^(?:/|", r"(?:/", SAFE_ID_REGEX_STR, ")+", r")$");
}

pub const ACL_PATH_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&ACL_PATH_REGEX);

pub const ACL_PATH_SCHEMA: Schema = StringSchema::new("Access control path.")
    .format(&ACL_PATH_FORMAT)
    .min_length(1)
    .max_length(128)
    .schema();

pub const ACL_PROPAGATE_SCHEMA: Schema =
    BooleanSchema::new("Allow to propagate (inherit) permissions.")
        .default(true)
        .schema();

#[api(
    properties: {
        user: {
            type: User,
            flatten: true,
        },
        tokens: {
            type: Array,
            optional: true,
            description: "List of user's API tokens.",
            items: {
                type: ApiToken
            },
        },
        "totp-locked": {
            type: bool,
            optional: true,
            default: false,
            description: "True if the user is currently locked out of TOTP factors",
        },
        "tfa-locked-until": {
            optional: true,
            description: "Contains a timestamp until when a user is locked out of 2nd factors",
        },
    }
)]
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// User properties with added list of ApiTokens
pub struct UserWithTokens {
    #[serde(flatten)]
    pub user: User,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tokens: Vec<ApiToken>,
    #[serde(skip_serializing_if = "bool_is_false", default)]
    pub totp_locked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tfa_locked_until: Option<i64>,
}

fn bool_is_false(b: &bool) -> bool {
    !b
}

#[api(
    properties: {
        tokenid: {
            schema: PROXMOX_TOKEN_ID_SCHEMA,
        },
        comment: {
            optional: true,
            schema: COMMENT_SCHEMA,
        },
        enable: {
            optional: true,
            schema: ENABLE_USER_SCHEMA,
        },
        expire: {
            optional: true,
            schema: EXPIRE_USER_SCHEMA,
        },
    }
)]
#[derive(Serialize, Deserialize, Clone, PartialEq)]
/// ApiToken properties.
pub struct ApiToken {
    pub tokenid: Authid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire: Option<i64>,
}

impl ApiToken {
    pub fn is_active(&self) -> bool {
        if !self.enable.unwrap_or(true) {
            return false;
        }
        if let Some(expire) = self.expire {
            let now = proxmox_time::epoch_i64();
            if expire > 0 && expire <= now {
                return false;
            }
        }
        true
    }
}

#[api]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// ApiToken id / secret pair
pub struct ApiTokenSecret {
    pub tokenid: Authid,
    /// The secret associated with the token.
    pub secret: String,
}

#[api(
    properties: {
        userid: {
            type: Userid,
        },
        comment: {
            optional: true,
            schema: COMMENT_SCHEMA,
        },
        enable: {
            optional: true,
            schema: ENABLE_USER_SCHEMA,
        },
        expire: {
            optional: true,
            schema: EXPIRE_USER_SCHEMA,
        },
        firstname: {
            optional: true,
            schema: FIRST_NAME_SCHEMA,
        },
        lastname: {
            schema: LAST_NAME_SCHEMA,
            optional: true,
         },
        email: {
            schema: EMAIL_SCHEMA,
            optional: true,
        },
    }
)]
#[derive(Serialize, Deserialize, Updater, PartialEq, Eq, Clone)]
/// User properties.
pub struct User {
    #[updater(skip)]
    pub userid: Userid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firstname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lastname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

impl User {
    pub fn is_active(&self) -> bool {
        if !self.enable.unwrap_or(true) {
            return false;
        }
        if let Some(expire) = self.expire {
            let now = proxmox_time::epoch_i64();
            if expire > 0 && expire <= now {
                return false;
            }
        }
        true
    }
}

#[api]
/// Type of the 'ugid' property in the ACL entry list.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AclUgidType {
    /// An entry for a user (or token).
    User,
    /// An entry for a group.
    Group,
}

serde_plain::derive_display_from_serialize!(AclUgidType);
serde_plain::derive_fromstr_from_deserialize!(AclUgidType);

#[api(
    properties: {
        propagate: { schema: ACL_PROPAGATE_SCHEMA, },
        path: { schema: ACL_PATH_SCHEMA, },
        ugid_type: { type: AclUgidType },
        ugid: {
            type: String,
            description: "User or Group ID.",
        },
    }
)]
#[derive(Serialize, Deserialize, PartialEq, Clone, Hash)]
/// Access control list entry.
pub struct AclListItem {
    pub path: String,
    pub ugid: String,
    pub ugid_type: AclUgidType,
    pub propagate: bool,
    /// A role represented as a string.
    pub roleid: String,
}

#[api(
    properties: {
        privs: {
            type: Array,
            description: "List of Privileges",
            items: {
                type: String,
                description: "A Privilege",
            },
        },
        comment: {
            schema: COMMENT_SCHEMA,
            optional: true,
        }
    }
)]
/// A struct that the describes a role and shows the associated privileges.
#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct RoleInfo {
    /// The id of the role
    pub roleid: String,
    /// The privileges the role holds
    pub privs: Vec<String>,
    /// A comment describing the role
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}
