use std::collections::HashSet;

use anyhow::{format_err, Context, Error};

use proxmox_access_control::acl::AclTree;
use proxmox_access_control::types::{
    ApiToken, User, EMAIL_SCHEMA, FIRST_NAME_SCHEMA, LAST_NAME_SCHEMA,
};
use proxmox_auth_api::types::{Authid, Realm, Userid};
use proxmox_product_config::ApiLockGuard;
use proxmox_schema::{ApiType, Schema};
use proxmox_section_config::SectionConfigData;

use crate::types::{
    AdRealmConfig, LdapRealmConfig, RemoveVanished, SyncAttributes, SyncDefaultsOptions,
    REMOVE_VANISHED_ARRAY, USER_CLASSES_ARRAY,
};
use crate::{Config, Connection, SearchResult};

/// Implementation for syncing Active Directory realms. Merely a thin wrapper over
/// `LdapRealmSyncJob`, as AD is just LDAP with some special requirements.
pub struct AdRealmSyncJob(LdapRealmSyncJob);

impl AdRealmSyncJob {
    pub fn new(
        realm: Realm,
        realm_config: AdRealmConfig,
        ldap_config: Config,
        override_settings: &GeneralSyncSettingsOverride,
        dry_run: bool,
    ) -> Result<Self, Error> {
        let sync_settings = GeneralSyncSettings::default()
            .apply_config(realm_config.sync_defaults_options.as_deref())?
            .apply_override(override_settings)?;
        let sync_attributes = LdapSyncSettings::new(
            "sAMAccountName",
            realm_config.sync_attributes.as_deref(),
            realm_config.user_classes.as_deref(),
            realm_config.filter.as_deref(),
        )?;

        Ok(Self(LdapRealmSyncJob {
            realm,
            general_sync_settings: sync_settings,
            ldap_sync_settings: sync_attributes,
            ldap_config,
            dry_run,
        }))
    }

    pub async fn sync(&self) -> Result<(), Error> {
        self.0.sync().await
    }
}

/// Implementation for syncing LDAP realms
pub struct LdapRealmSyncJob {
    realm: Realm,
    general_sync_settings: GeneralSyncSettings,
    ldap_sync_settings: LdapSyncSettings,
    ldap_config: Config,
    dry_run: bool,
}

impl LdapRealmSyncJob {
    /// Create new LdapRealmSyncJob
    pub fn new(
        realm: Realm,
        realm_config: LdapRealmConfig,
        ldap_config: Config,
        override_settings: &GeneralSyncSettingsOverride,
        dry_run: bool,
    ) -> Result<Self, Error> {
        let general_sync_settings = GeneralSyncSettings::default()
            .apply_config(realm_config.sync_defaults_options.as_deref())?
            .apply_override(override_settings)?;

        let ldap_sync_settings = LdapSyncSettings::new(
            &realm_config.user_attr,
            realm_config.sync_attributes.as_deref(),
            realm_config.user_classes.as_deref(),
            realm_config.filter.as_deref(),
        )?;

        Ok(Self {
            realm,
            general_sync_settings,
            ldap_sync_settings,
            ldap_config,
            dry_run,
        })
    }

    /// Perform realm synchronization
    pub async fn sync(&self) -> Result<(), Error> {
        if self.dry_run {
            log::info!("this is a DRY RUN - changes will not be persisted");
        }

        let ldap = Connection::new(self.ldap_config.clone());

        let parameters = crate::SearchParameters {
            attributes: self.ldap_sync_settings.attributes.clone(),
            user_classes: self.ldap_sync_settings.user_classes.clone(),
            user_filter: self.ldap_sync_settings.user_filter.clone(),
        };

        let users = ldap.search_entities(&parameters).await?;
        self.update_user_config(&users)?;

        Ok(())
    }

    fn update_user_config(&self, users: &[SearchResult]) -> Result<(), Error> {
        let user_lock = proxmox_access_control::user::lock_config()?;
        let acl_lock = proxmox_access_control::acl::lock_config()?;

        let (mut user_config, _digest) = proxmox_access_control::user::config()?;
        let (mut tree, _) = proxmox_access_control::acl::config()?;

        let retrieved_users = self.create_or_update_users(&mut user_config, &user_lock, users)?;

        if self.general_sync_settings.should_remove_entries() {
            let vanished_users =
                self.compute_vanished_users(&user_config, &user_lock, &retrieved_users)?;

            self.delete_users(
                &mut user_config,
                &user_lock,
                &mut tree,
                &acl_lock,
                &vanished_users,
            )?;
        }

        if !self.dry_run {
            proxmox_access_control::user::save_config(&user_config)
                .context("could not store user config")?;
            proxmox_access_control::acl::save_config(&tree)
                .context("could not store acl config")?;
        }

        Ok(())
    }

    fn create_or_update_users(
        &self,
        user_config: &mut SectionConfigData,
        _user_lock: &ApiLockGuard,
        users: &[SearchResult],
    ) -> Result<HashSet<Userid>, Error> {
        let mut retrieved_users = HashSet::new();

        for result in users {
            let user_id_attribute = &self.ldap_sync_settings.user_attr;

            let result = {
                let username = result
                    .attributes
                    .get(user_id_attribute)
                    .ok_or_else(|| {
                        format_err!(
                            "userid attribute `{user_id_attribute}` not in LDAP search result"
                        )
                    })?
                    .first()
                    .context("userid attribute array is empty")?
                    .clone();

                let username = format!("{username}@{realm}", realm = self.realm.as_str());

                let userid: Userid = username
                    .parse()
                    .map_err(|err| format_err!("could not parse username `{username}` - {err}"))?;
                retrieved_users.insert(userid.clone());

                self.create_or_update_user(user_config, &userid, result)?;
                anyhow::Ok(())
            };

            if let Err(e) = result {
                log::info!("could not create/update user: {e}");
            }
        }

        Ok(retrieved_users)
    }

    fn create_or_update_user(
        &self,
        user_config: &mut SectionConfigData,
        userid: &Userid,
        result: &SearchResult,
    ) -> Result<(), Error> {
        let existing_user = user_config.lookup::<User>("user", userid.as_str()).ok();
        let new_or_updated_user =
            self.construct_or_update_user(result, userid, existing_user.as_ref());

        if let Some(existing_user) = existing_user {
            if existing_user != new_or_updated_user {
                log::info!("updating user {}", new_or_updated_user.userid.as_str());
            }
        } else {
            log::info!("creating user {}", new_or_updated_user.userid.as_str());
        }

        user_config.set_data(
            new_or_updated_user.userid.as_str(),
            "user",
            &new_or_updated_user,
        )?;
        Ok(())
    }

    fn construct_or_update_user(
        &self,
        result: &SearchResult,
        userid: &Userid,
        existing_user: Option<&User>,
    ) -> User {
        let lookup = |attribute: &str, ldap_attribute: Option<&String>, schema: &'static Schema| {
            let value = result.attributes.get(ldap_attribute?)?.first()?;
            let schema = schema.unwrap_string_schema();

            if let Err(e) = schema.check_constraints(value) {
                log::warn!("{userid}: ignoring attribute `{attribute}`: {e}");

                None
            } else {
                Some(value.clone())
            }
        };

        User {
            userid: userid.clone(),
            comment: existing_user.as_ref().and_then(|u| u.comment.clone()),
            enable: existing_user
                .and_then(|o| o.enable)
                .or(Some(self.general_sync_settings.enable_new)),
            expire: existing_user.and_then(|u| u.expire).or(Some(0)),
            firstname: lookup(
                "firstname",
                self.ldap_sync_settings.firstname_attr.as_ref(),
                &FIRST_NAME_SCHEMA,
            )
            .or_else(|| {
                if !self.general_sync_settings.should_remove_properties() {
                    existing_user.and_then(|o| o.firstname.clone())
                } else {
                    None
                }
            }),
            lastname: lookup(
                "lastname",
                self.ldap_sync_settings.lastname_attr.as_ref(),
                &LAST_NAME_SCHEMA,
            )
            .or_else(|| {
                if !self.general_sync_settings.should_remove_properties() {
                    existing_user.and_then(|o| o.lastname.clone())
                } else {
                    None
                }
            }),
            email: lookup(
                "email",
                self.ldap_sync_settings.email_attr.as_ref(),
                &EMAIL_SCHEMA,
            )
            .or_else(|| {
                if !self.general_sync_settings.should_remove_properties() {
                    existing_user.and_then(|o| o.email.clone())
                } else {
                    None
                }
            }),
        }
    }

    fn compute_vanished_users(
        &self,
        user_config: &SectionConfigData,
        _user_lock: &ApiLockGuard,
        synced_users: &HashSet<Userid>,
    ) -> Result<Vec<Userid>, Error> {
        Ok(user_config
            .convert_to_typed_array::<User>("user")?
            .into_iter()
            .filter(|user| {
                user.userid.realm() == self.realm && !synced_users.contains(&user.userid)
            })
            .map(|user| user.userid)
            .collect())
    }

    fn delete_users(
        &self,
        user_config: &mut SectionConfigData,
        _user_lock: &ApiLockGuard,
        acl_config: &mut AclTree,
        _acl_lock: &ApiLockGuard,
        to_delete: &[Userid],
    ) -> Result<(), Error> {
        for userid in to_delete {
            log::info!("deleting user {}", userid.as_str());

            // Delete the user
            user_config.sections.remove(userid.as_str());

            if self.general_sync_settings.should_remove_acls() {
                let auth_id = userid.clone().into();
                // Delete the user's ACL entries
                acl_config.delete_authid(&auth_id);
            }

            let user_tokens: Vec<ApiToken> = user_config
                .convert_to_typed_array::<ApiToken>("token")?
                .into_iter()
                .filter(|token| token.tokenid.user().eq(userid))
                .collect();

            // Delete tokens, token secrets and ACLs corresponding to all tokens for a user
            for token in user_tokens {
                if let Some(name) = token.tokenid.tokenname() {
                    let tokenid = Authid::from((userid.clone(), Some(name.to_owned())));
                    let tokenid_string = tokenid.to_string();

                    user_config.sections.remove(&tokenid_string);

                    if !self.dry_run {
                        if let Err(e) =
                            proxmox_access_control::token_shadow::delete_secret(&tokenid)
                        {
                            log::warn!("could not delete token for user {userid}: {e}",)
                        }
                    }

                    if self.general_sync_settings.should_remove_acls() {
                        acl_config.delete_authid(&tokenid);
                    }
                }
            }
        }

        Ok(())
    }
}

/// General realm sync settings - Override for manual invocation
pub struct GeneralSyncSettingsOverride {
    pub remove_vanished: Option<String>,
    pub enable_new: Option<bool>,
}

/// General realm sync settings from the realm configuration
struct GeneralSyncSettings {
    remove_vanished: Vec<RemoveVanished>,
    enable_new: bool,
}

/// LDAP-specific realm sync settings from the realm configuration
struct LdapSyncSettings {
    user_attr: String,
    firstname_attr: Option<String>,
    lastname_attr: Option<String>,
    email_attr: Option<String>,
    attributes: Vec<String>,
    user_classes: Vec<String>,
    user_filter: Option<String>,
}

impl LdapSyncSettings {
    fn new(
        user_attr: &str,
        sync_attributes: Option<&str>,
        user_classes: Option<&str>,
        user_filter: Option<&str>,
    ) -> Result<Self, Error> {
        let mut attributes = vec![user_attr.to_owned()];

        let mut email = None;
        let mut firstname = None;
        let mut lastname = None;

        if let Some(sync_attributes) = &sync_attributes {
            let value = SyncAttributes::API_SCHEMA.parse_property_string(sync_attributes)?;
            let sync_attributes: SyncAttributes = serde_json::from_value(value)?;

            email.clone_from(&sync_attributes.email);
            firstname.clone_from(&sync_attributes.firstname);
            lastname.clone_from(&sync_attributes.lastname);

            if let Some(email_attr) = &sync_attributes.email {
                attributes.push(email_attr.clone());
            }

            if let Some(firstname_attr) = &sync_attributes.firstname {
                attributes.push(firstname_attr.clone());
            }

            if let Some(lastname_attr) = &sync_attributes.lastname {
                attributes.push(lastname_attr.clone());
            }
        }

        let user_classes = if let Some(user_classes) = &user_classes {
            let a = USER_CLASSES_ARRAY.parse_property_string(user_classes)?;
            serde_json::from_value(a)?
        } else {
            vec![
                "posixaccount".into(),
                "person".into(),
                "inetorgperson".into(),
                "user".into(),
            ]
        };

        Ok(Self {
            user_attr: user_attr.to_owned(),
            firstname_attr: firstname,
            lastname_attr: lastname,
            email_attr: email,
            attributes,
            user_classes,
            user_filter: user_filter.map(ToOwned::to_owned),
        })
    }
}

impl Default for GeneralSyncSettings {
    fn default() -> Self {
        Self {
            remove_vanished: Default::default(),
            enable_new: true,
        }
    }
}

impl GeneralSyncSettings {
    fn apply_config(self, sync_defaults_options: Option<&str>) -> Result<Self, Error> {
        let mut enable_new = None;
        let mut remove_vanished = None;

        if let Some(sync_defaults_options) = sync_defaults_options {
            let sync_defaults_options = Self::parse_sync_defaults_options(sync_defaults_options)?;

            enable_new = sync_defaults_options.enable_new;

            if let Some(vanished) = sync_defaults_options.remove_vanished.as_deref() {
                remove_vanished = Some(Self::parse_remove_vanished(vanished)?);
            }
        }

        Ok(Self {
            enable_new: enable_new.unwrap_or(self.enable_new),
            remove_vanished: remove_vanished.unwrap_or(self.remove_vanished),
        })
    }

    fn apply_override(self, override_config: &GeneralSyncSettingsOverride) -> Result<Self, Error> {
        let enable_new = override_config.enable_new;
        let remove_vanished = if let Some(s) = override_config.remove_vanished.as_deref() {
            Some(Self::parse_remove_vanished(s)?)
        } else {
            None
        };

        Ok(Self {
            enable_new: enable_new.unwrap_or(self.enable_new),
            remove_vanished: remove_vanished.unwrap_or(self.remove_vanished),
        })
    }

    fn parse_sync_defaults_options(s: &str) -> Result<SyncDefaultsOptions, Error> {
        let value = SyncDefaultsOptions::API_SCHEMA.parse_property_string(s)?;
        Ok(serde_json::from_value(value)?)
    }

    fn parse_remove_vanished(s: &str) -> Result<Vec<RemoveVanished>, Error> {
        Ok(serde_json::from_value(
            REMOVE_VANISHED_ARRAY.parse_property_string(s)?,
        )?)
    }

    fn should_remove_properties(&self) -> bool {
        self.remove_vanished.contains(&RemoveVanished::Properties)
    }

    fn should_remove_entries(&self) -> bool {
        self.remove_vanished.contains(&RemoveVanished::Entry)
    }

    fn should_remove_acls(&self) -> bool {
        self.remove_vanished.contains(&RemoveVanished::Acl)
    }
}
