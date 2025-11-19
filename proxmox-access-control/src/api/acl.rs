use anyhow::{bail, format_err, Error};

use proxmox_auth_api::types::{Authid, PROXMOX_GROUP_ID_SCHEMA};
use proxmox_config_digest::{ConfigDigest, PROXMOX_CONFIG_DIGEST_SCHEMA};
use proxmox_router::{Permission, Router, RpcEnvironment};
use proxmox_schema::api;

use crate::acl::AclTreeNode;
use crate::init::access_conf;
use crate::types::{AclListItem, AclUgidType, RoleInfo, ACL_PATH_SCHEMA, ACL_PROPAGATE_SCHEMA};
use crate::CachedUserInfo;

#[api(
    input: {
        properties: {
            path: {
                schema: ACL_PATH_SCHEMA,
                optional: true,
            },
            exact: {
                description: "If set, returns only ACL for the exact path.",
                type: bool,
                optional: true,
                default: false,
            },
            "all-for-authid": {
                description: "Whether to return all ACL entries for the exact current authid only. \
                    All ACL entries will appear as `AclUgidType::User` ACLs, regardles of whether \
                    they are stored as user or group entries. Hence, when using this parameter this \
                    endpoint cannot be used to retrieve information for updating the ACL tree \
                    directly.",
                type: bool,
                optional: true,
                default: false,
            }
        },
    },
    returns: {
        description: "ACL entry list.",
        type: Array,
        items: {
            type: AclListItem,
        }
    },
    access: {
        permission: &Permission::Anybody,
        description: "Returns all ACLs if a user has sufficient privileges on this endpoint. \
            Otherwise it is limited to the user's API tokens. However, if `all-for-authid` is \
            specified, all ACLs of the current Authid will be returned, whether the Authid has \
            privileges to list other ACLs here or not.",
    },
)]
/// Get ACL entries, can be filter by path.
pub fn read_acl(
    path: Option<String>,
    exact: bool,
    all_for_authid: bool,
    rpcenv: &mut dyn RpcEnvironment,
) -> Result<Vec<AclListItem>, Error> {
    let auth_id = rpcenv
        .get_auth_id()
        .ok_or_else(|| format_err!("endpoint called without an auth id"))?
        .parse()?;

    // if a user does not have enough permissions to see all entries, we need to filter them
    let filter_entries = CachedUserInfo::new()?
        .check_privs(
            &auth_id,
            &["access", "acl"],
            access_conf().acl_audit_privileges(),
            access_conf().allow_partial_permission_match(),
        )
        .is_err();

    let filter = if filter_entries || all_for_authid {
        Some(auth_id)
    } else {
        None
    };

    let (mut tree, digest) = crate::acl::config()?;

    let node = if let Some(path) = &path {
        if let Some(node) = tree.find_node(path) {
            node
        } else {
            return Ok(Vec::new());
        }
    } else {
        &tree.root
    };

    rpcenv["digest"] = hex::encode(digest).into();

    Ok(extract_acl_node_data(
        node,
        path.as_deref(),
        all_for_authid,
        exact,
        &filter,
    ))
}

#[api(
    protected: true,
    input: {
        properties: {
            path: {
                schema: ACL_PATH_SCHEMA,
            },
            role: {
                type: String,
                description: "Name of a role that the auth id will be granted.",
            },
            propagate: {
                optional: true,
                schema: ACL_PROPAGATE_SCHEMA,
            },
            "auth-id": {
                optional: true,
                type: Authid,
            },
            group: {
                optional: true,
                schema: PROXMOX_GROUP_ID_SCHEMA,
            },
            delete: {
                optional: true,
                description: "Remove permissions (instead of adding it).",
                type: bool,
                default: false,
            },
            digest: {
                optional: true,
                schema: PROXMOX_CONFIG_DIGEST_SCHEMA,
            },
       },
    },
    access: {
        permission: &Permission::Anybody,
        description: "Requires sufficient permissions to edit the ACL, otherwise only editing the current user's API token permissions is allowed."
    },
)]
/// Update ACL
#[allow(clippy::too_many_arguments)]
pub fn update_acl(
    path: String,
    role: String,
    propagate: Option<bool>,
    auth_id: Option<Authid>,
    group: Option<String>,
    delete: bool,
    digest: Option<ConfigDigest>,
    rpcenv: &mut dyn RpcEnvironment,
) -> Result<(), Error> {
    let access_conf = access_conf();

    if !access_conf.roles().contains_key(role.as_str()) {
        bail!("Role does not exist, please make sure to specify a valid role!")
    }

    let current_auth_id: Authid = rpcenv
        .get_auth_id()
        .expect("auth id could not be determined")
        .parse()?;

    let unprivileged_user = CachedUserInfo::new()?
        .check_privs(
            &current_auth_id,
            &["access", "acl"],
            access_conf.acl_modify_privileges(),
            access_conf.allow_partial_permission_match(),
        )
        .is_err();

    if unprivileged_user {
        if group.is_none()
            && !current_auth_id.is_token()
            && auth_id
                .as_ref()
                .map(|id| id.is_token() && current_auth_id.user() == id.user())
                .unwrap_or_default()
        {
            // a user is directly editing the privileges of their own tokens, this is always
            // allowed
        } else {
            if group.is_some() {
                bail!("Unprivileged users are not allowed to create group ACL item.");
            }

            let auth_id = auth_id.as_ref().ok_or_else(|| {
                format_err!("Unprivileged user needs to provide auth_id to update ACL item.")
            })?;

            if current_auth_id.is_token() {
                bail!("Unprivileged API tokens can't set ACL items.");
            }

            if !auth_id.is_token() {
                bail!("Unprivileged users can only set ACL items for API tokens.");
            }

            if current_auth_id.user() != auth_id.user() {
                bail!("Unprivileged users can only set ACL items for their own API tokens.");
            }

            // this should not be reachable, but just in case, bail here
            bail!("Unprivileged user is trying to set an invalid ACL item.")
        }
    }

    if let Some(auth_id) = &auth_id {
        // only allow deleting non-existing auth id's, not adding them
        if !delete {
            let exists = crate::user::cached_config()?
                .sections
                .contains_key(&auth_id.to_string());

            if !exists {
                if auth_id.is_token() {
                    bail!("no such API token");
                } else {
                    bail!("no such user.")
                }
            }
        }
    } else if group.is_some() {
        // FIXME: add support for groups
        bail!("parameter 'group' - groups are currently not supported");
    } else {
        // FIXME: suggest groups here once they exist
        bail!("missing 'userid' parameter");
    }

    // allow deleting invalid acl paths
    if !delete {
        access_conf.check_acl_path(&path)?;
    }

    let _guard = crate::acl::lock_config()?;
    let (mut tree, expected_digest) = crate::acl::config()?;
    expected_digest.detect_modification(digest.as_ref())?;

    let propagate = propagate.unwrap_or(true);

    if let Some(auth_id) = &auth_id {
        if delete {
            tree.delete_user_role(&path, auth_id, &role);
        } else {
            tree.insert_user_role(&path, auth_id, &role, propagate);
        }
    } else if let Some(group) = &group {
        if delete {
            tree.delete_group_role(&path, group, &role);
        } else {
            tree.insert_group_role(&path, group, &role, propagate);
        }
    }

    crate::acl::save_config(&tree)?;

    Ok(())
}

fn extract_acl_node_data(
    node: &AclTreeNode,
    path: Option<&str>,
    all_for_authid: bool,
    exact: bool,
    auth_id_filter: &Option<Authid>,
) -> Vec<AclListItem> {
    // tokens can't have tokens, so we can early return
    if let Some(auth_id_filter) = auth_id_filter {
        if auth_id_filter.is_token() {
            return Vec::new();
        }
    }

    let mut to_return = Vec::new();
    let mut nodes = vec![(path.unwrap_or("").to_string(), node)];

    while let Some((path, node)) = nodes.pop() {
        let path_str = if path.is_empty() { "/" } else { &path };

        if all_for_authid {
            if let Some(auth_id) = auth_id_filter {
                // this will extract all roles for `auth_id_filer` from the node. group acls will
                // be handled according to the acl trees implementation. we mask them here as user
                // ACLs to avoid disclosing more information than necessary.
                //
                // by setting `leaf` to true we always get all roles for this `auth_id` on the
                // current node.
                for (role, propagate) in node.extract_roles(auth_id, true) {
                    to_return.push(AclListItem {
                        path: path_str.to_owned(),
                        propagate,
                        ugid_type: AclUgidType::User,
                        ugid: auth_id.to_string(),
                        roleid: role.to_string(),
                    })
                }
            }
        } else {
            for (user, roles) in &node.users {
                if let Some(auth_id_filter) = auth_id_filter {
                    if !user.is_token() || user.user() != auth_id_filter.user() {
                        continue;
                    }
                }

                for (role, propagate) in roles {
                    to_return.push(AclListItem {
                        path: path_str.to_owned(),
                        propagate: *propagate,
                        ugid_type: AclUgidType::User,
                        ugid: user.to_string(),
                        roleid: role.to_string(),
                    });
                }
            }

            for (group, roles) in &node.groups {
                if auth_id_filter.is_some() {
                    continue;
                }

                for (role, propagate) in roles {
                    to_return.push(AclListItem {
                        path: path_str.to_owned(),
                        propagate: *propagate,
                        ugid_type: AclUgidType::Group,
                        ugid: group.to_string(),
                        roleid: role.to_string(),
                    });
                }
            }
        }

        if !exact {
            nodes.extend(
                node.children
                    .iter()
                    .map(|(comp, child)| (format!("{path}/{comp}"), child)),
            );
        }
    }

    to_return
}

pub const ACL_ROUTER: Router = Router::new()
    .get(&API_METHOD_READ_ACL)
    .put(&API_METHOD_UPDATE_ACL);

#[api(
    returns: {
        description: "List of roles.",
        type: Array,
        items: {
            type: RoleInfo,
        }
    },
    access: {
        permission: &Permission::Anybody,
    }
)]
/// A list of available roles
fn list_roles() -> Result<Vec<RoleInfo>, Error> {
    let list = access_conf()
        .roles()
        .iter()
        .map(|(role, (privs, comment))| {
            let priv_list = access_conf()
                .privileges()
                .iter()
                .filter_map(|(name, privilege)| {
                    if privs & privilege > 0 {
                        Some(name.to_string())
                    } else {
                        None
                    }
                });

            RoleInfo {
                roleid: role.to_string(),
                privs: priv_list.collect(),
                comment: Some(comment.to_string()),
            }
        });

    Ok(list.collect())
}

pub const ROLE_ROUTER: Router = Router::new().get(&API_METHOD_LIST_ROLES);
