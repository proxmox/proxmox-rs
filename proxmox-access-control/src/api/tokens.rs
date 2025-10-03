//! User Management

use anyhow::{bail, Error};
use proxmox_config_digest::ConfigDigest;
use proxmox_schema::api_types::COMMENT_SCHEMA;

use proxmox_auth_api::types::{Authid, Tokenname, Userid};
use proxmox_router::{ApiMethod, RpcEnvironment};
use proxmox_schema::api;

use crate::acl;
use crate::token_shadow::{self};
use crate::types::{
    ApiToken, ApiTokenSecret, DeletableTokenProperty, TokenApiEntry, ENABLE_USER_SCHEMA,
    EXPIRE_USER_SCHEMA, REGENERATE_TOKEN_SCHEMA,
};

#[api(
    input: {
        properties: {
            userid: {
                type: Userid,
            },
            "token-name": {
                type: Tokenname,
            },
        },
    },
    returns: { type: ApiToken },
)]
/// Read user's API token metadata
pub fn read_token(
    userid: Userid,
    token_name: Tokenname,
    _info: &ApiMethod,
    rpcenv: &mut dyn RpcEnvironment,
) -> Result<ApiToken, Error> {
    let (config, digest) = crate::user::config()?;

    let tokenid = Authid::from((userid, Some(token_name)));

    rpcenv["digest"] = hex::encode(digest).into();
    config.lookup("token", &tokenid.to_string())
}

#[api(
    protected: true,
    input: {
        properties: {
            userid: {
                type: Userid,
            },
            "token-name": {
                type: Tokenname,
            },
            comment: {
                optional: true,
                schema: COMMENT_SCHEMA,
            },
            enable: {
                schema: ENABLE_USER_SCHEMA,
                optional: true,
            },
            expire: {
                schema: EXPIRE_USER_SCHEMA,
                optional: true,
            },
            digest: {
                optional: true,
                type: ConfigDigest,
            },
        },
    },
    returns: { type: ApiTokenSecret },
)]
/// Generate a new API token with given metadata
pub fn generate_token(
    userid: Userid,
    token_name: Tokenname,
    comment: Option<String>,
    enable: Option<bool>,
    expire: Option<i64>,
    digest: Option<ConfigDigest>,
) -> Result<ApiTokenSecret, Error> {
    let _lock = crate::user::lock_config()?;
    let (mut config, config_digest) = crate::user::config()?;

    config_digest.detect_modification(digest.as_ref())?;

    let tokenid = Authid::from((userid.clone(), Some(token_name.clone())));
    let tokenid_string = tokenid.to_string();

    if config.sections.contains_key(&tokenid_string) {
        bail!(
            "token '{}' for user '{userid}' already exists.",
            token_name.as_str(),
        );
    }

    let secret = token_shadow::generate_and_set_secret(&tokenid)?;

    let token = ApiToken {
        tokenid: tokenid.clone(),
        comment,
        enable,
        expire,
    };

    config.set_data(&tokenid_string, "token", &token)?;

    crate::user::save_config(&config)?;

    Ok(ApiTokenSecret { tokenid, secret })
}

#[api(
    protected: true,
    input: {
        properties: {
            userid: {
                type: Userid,
            },
            "token-name": {
                type: Tokenname,
            },
            comment: {
                optional: true,
                schema: COMMENT_SCHEMA,
            },
            enable: {
                schema: ENABLE_USER_SCHEMA,
                optional: true,
            },
            expire: {
                schema: EXPIRE_USER_SCHEMA,
                optional: true,
            },
            regenerate: {
                schema: REGENERATE_TOKEN_SCHEMA,
                optional: true,
            },
            delete: {
                description: "List of properties to delete.",
                type: Array,
                optional: true,
                items: {
                    type: DeletableTokenProperty,
                }
            },
            digest: {
                optional: true,
                type: ConfigDigest,
            },
        },
    },
    returns: {
        type: ApiTokenSecret,
        optional: true
    }
)]
/// Update user's API token metadata. If regenerate is set to true, the token and it's new secret
/// will be returned.
#[allow(clippy::too_many_arguments)]

pub fn update_token(
    userid: Userid,
    token_name: Tokenname,
    comment: Option<String>,
    enable: Option<bool>,
    expire: Option<i64>,
    regenerate: Option<bool>,
    delete: Option<Vec<DeletableTokenProperty>>,
    digest: Option<ConfigDigest>,
) -> Result<Option<ApiTokenSecret>, Error> {
    let _lock = crate::user::lock_config()?;

    let (mut config, config_digest) = crate::user::config()?;
    config_digest.detect_modification(digest.as_ref())?;

    let tokenid = Authid::from((userid, Some(token_name)));
    let tokenid_string = tokenid.to_string();

    let mut data: ApiToken = config.lookup("token", &tokenid_string)?;

    if let Some(delete) = delete {
        for delete_prop in delete {
            match delete_prop {
                DeletableTokenProperty::Comment => data.comment = None,
            }
        }
    }

    if let Some(comment) = comment {
        let comment = comment.trim().to_string();
        if comment.is_empty() {
            data.comment = None;
        } else {
            data.comment = Some(comment);
        }
    }

    if let Some(enable) = enable {
        data.enable = if enable { None } else { Some(false) };
    }

    if let Some(expire) = expire {
        data.expire = if expire > 0 { Some(expire) } else { None };
    }

    let new_secret = if regenerate == Some(true) {
        let secret = token_shadow::generate_and_set_secret(&tokenid)?;
        Some(ApiTokenSecret { tokenid, secret })
    } else {
        None
    };

    config.set_data(&tokenid_string, "token", &data)?;

    crate::user::save_config(&config)?;

    Ok(new_secret)
}

#[api(
    protected: true,
    input: {
        properties: {
            userid: {
                type: Userid,
            },
            "token-name": {
                type: Tokenname,
            },
            digest: {
                optional: true,
                type: ConfigDigest,
            },
        },
    },
)]
/// Delete a user's API token
pub fn delete_token(
    userid: Userid,
    token_name: Tokenname,
    digest: Option<ConfigDigest>,
) -> Result<(), Error> {
    let _acl_lock = acl::lock_config()?;
    let _user_lock = crate::user::lock_config()?;

    let (mut user_config, config_digest) = crate::user::config()?;
    config_digest.detect_modification(digest.as_ref())?;
    let (mut acl_config, _digest) = crate::acl::config()?;

    let tokenid = Authid::from((userid.clone(), Some(token_name.clone())));
    let tokenid_string = tokenid.to_string();

    if user_config.sections.remove(&tokenid_string).is_none() {
        bail!(
            "token '{}' of user '{userid}' does not exist.",
            token_name.as_str(),
        );
    }

    token_shadow::delete_secret(&tokenid)?;
    acl_config.delete_authid(&tokenid);
    crate::user::save_config(&user_config)?;
    crate::acl::save_config(&acl_config)?;

    Ok(())
}

#[api(
    input: {
        properties: {
            userid: {
                type: Userid,
            },
        },
    },
    returns: {
        description: "List user's API tokens (with config digest).",
        type: Array,
        items: { type: TokenApiEntry },
    },
)]
/// List user's API tokens
pub fn list_tokens(
    userid: Userid,
    _info: &ApiMethod,
    rpcenv: &mut dyn RpcEnvironment,
) -> Result<Vec<TokenApiEntry>, Error> {
    let (config, digest) = crate::user::config()?;

    let list: Vec<ApiToken> = config.convert_to_typed_array("token")?;

    rpcenv["digest"] = hex::encode(digest).into();

    let filter_by_owner = |token: ApiToken| {
        if token.tokenid.is_token() && token.tokenid.user() == &userid {
            let token_name = token.tokenid.tokenname().unwrap().to_owned();
            Some(TokenApiEntry { token_name, token })
        } else {
            None
        }
    };

    let res = list.into_iter().filter_map(filter_by_owner).collect();

    Ok(res)
}
