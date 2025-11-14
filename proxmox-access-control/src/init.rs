use std::collections::HashMap;
use std::sync::OnceLock;

use anyhow::{format_err, Error};

use proxmox_auth_api::types::{Authid, Userid};
use proxmox_section_config::SectionConfigData;

static ACCESS_CONF: OnceLock<&'static dyn AccessControlConfig> = OnceLock::new();

/// This trait specifies the functions a product needs to implement to get ACL tree based access
/// control management from this plugin.
pub trait AccessControlConfig: Send + Sync {
    /// Returns a mapping of all recognized privileges and their corresponding `u64` value.
    fn privileges(&self) -> &HashMap<&str, u64>;

    /// Returns a mapping of all recognized roles and their corresponding `u64` value as well as
    /// a comment.
    fn roles(&self) -> &HashMap<&str, (u64, &str)>;

    /// Checks whether an `Authid` has super user privileges or not.
    ///
    /// Default: Always returns `false`.
    fn is_superuser(&self, _auth_id: &Authid) -> bool {
        false
    }

    /// Checks whether a user is part of a group.
    ///
    /// Default: Always returns `false`.
    fn is_group_member(&self, _user_id: &Userid, _group: &str) -> bool {
        false
    }

    /// Returns the current cache generation of the user and acl configs. If the generation was
    /// incremented since the last time the cache was queried, the configs are loaded again from
    /// disk.
    ///
    /// Returning `None` will always reload the cache.
    ///
    /// Default: Always returns `None`.
    fn cache_generation(&self) -> Option<usize> {
        None
    }

    /// Increment the cache generation of user and acl configs. This indicates that they were
    /// changed on disk.
    ///
    /// Default: Does nothing.
    fn increment_cache_generation(&self) -> Result<(), Error> {
        Ok(())
    }

    /// Optionally returns a role that has no access to any resource.
    ///
    /// Default: Returns `None`.
    fn role_no_access(&self) -> Option<&str> {
        None
    }

    /// Optionally returns a role that is allowed to access all resources.
    ///
    /// Default: Returns `None`.
    fn role_admin(&self) -> Option<&str> {
        None
    }

    /// Called after the user configuration is loaded to potentially re-add fixed users, such as a
    /// `root@pam` user.
    fn init_user_config(&self, config: &mut SectionConfigData) -> Result<(), Error> {
        let _ = config;
        Ok(())
    }

    /// This is used to determined what access control list entries a user is allowed to read.
    ///
    /// Override this if you want to use the `api` feature.
    fn acl_audit_privileges(&self) -> u64 {
        0
    }

    /// This is used to determine what privileges are needed to modify the access control list.
    ///
    /// Override this if you want to use the `api` feature.
    fn acl_modify_privileges(&self) -> u64 {
        0
    }

    /// Used to determine which paths are valid in a given `AclTree`.
    ///
    /// Override this if you want to use the `api` feature.
    fn check_acl_path(&self, path: &str) -> Result<(), Error> {
        let _ = path;
        Ok(())
    }

    /// Whether the API endpoints to inspect the ACL should use partial permission matching or not.
    ///
    /// Override this if the product in question uses more than one bit to specify permissions (so,
    /// in case it is *not* using a bitmap) and the match between permissions needs to be exact.
    fn allow_partial_permission_match(&self) -> bool {
        true
    }
}

pub fn init_access_config(config: &'static dyn AccessControlConfig) -> Result<(), Error> {
    ACCESS_CONF
        .set(config)
        .map_err(|_e| format_err!("cannot initialize acl tree config twice!"))
}

pub(crate) fn access_conf() -> &'static dyn AccessControlConfig {
    *ACCESS_CONF
        .get()
        .expect("please initialize the acm config before using it!")
}

#[cfg(feature = "impl")]
pub use impl_feature::init;

#[cfg(feature = "impl")]
pub(crate) mod impl_feature {
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;

    use anyhow::{format_err, Error};

    use crate::init::{init_access_config, AccessControlConfig};

    static ACCESS_CONF_DIR: OnceLock<PathBuf> = OnceLock::new();

    pub fn init<P: AsRef<Path>>(
        acm_config: &'static dyn AccessControlConfig,
        config_dir: P,
    ) -> Result<(), Error> {
        init_access_config(acm_config)?;
        init_access_config_dir(config_dir)
    }

    pub(crate) fn init_access_config_dir<P: AsRef<Path>>(config_dir: P) -> Result<(), Error> {
        ACCESS_CONF_DIR
            .set(config_dir.as_ref().to_owned())
            .map_err(|_e| format_err!("cannot initialize acl tree config twice!"))
    }

    fn conf_dir() -> &'static PathBuf {
        ACCESS_CONF_DIR
            .get()
            .expect("please initialize acm config dir before using it!")
    }

    pub(crate) fn acl_config() -> PathBuf {
        conf_dir().join("acl.cfg")
    }

    pub(crate) fn acl_config_lock() -> PathBuf {
        conf_dir().join(".acl.lck")
    }

    pub(crate) fn user_config() -> PathBuf {
        conf_dir().join("user.cfg")
    }

    pub(crate) fn user_config_lock() -> PathBuf {
        conf_dir().join(".user.lck")
    }

    pub(crate) fn token_shadow() -> PathBuf {
        conf_dir().join("token.shadow")
    }

    pub(crate) fn token_shadow_lock() -> PathBuf {
        conf_dir().join("token.shadow.lock")
    }
}
