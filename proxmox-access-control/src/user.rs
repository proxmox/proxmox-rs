use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use anyhow::{bail, Error};

use proxmox_auth_api::types::Authid;
use proxmox_config_digest::ConfigDigest;
use proxmox_product_config::{open_api_lockfile, replace_privileged_config, ApiLockGuard};
use proxmox_schema::*;
use proxmox_section_config::{SectionConfig, SectionConfigData, SectionConfigPlugin};

use crate::init::access_conf;
use crate::init::impl_feature::{user_config, user_config_lock};
use crate::types::{ApiToken, User};

fn get_or_init_config() -> &'static SectionConfig {
    static CONFIG: OnceLock<SectionConfig> = OnceLock::new();
    CONFIG.get_or_init(|| {
        let mut config = SectionConfig::new(&Authid::API_SCHEMA);

        let user_schema = match User::API_SCHEMA {
            Schema::Object(ref user_schema) => user_schema,
            _ => unreachable!(),
        };
        let user_plugin =
            SectionConfigPlugin::new("user".to_string(), Some("userid".to_string()), user_schema);
        config.register_plugin(user_plugin);

        let token_schema = match ApiToken::API_SCHEMA {
            Schema::Object(ref token_schema) => token_schema,
            _ => unreachable!(),
        };
        let token_plugin = SectionConfigPlugin::new(
            "token".to_string(),
            Some("tokenid".to_string()),
            token_schema,
        );
        config.register_plugin(token_plugin);

        config
    })
}

/// Get exclusive lock
pub fn lock_config() -> Result<ApiLockGuard, Error> {
    open_api_lockfile(user_config_lock(), None, true)
}

pub fn config() -> Result<(SectionConfigData, ConfigDigest), Error> {
    let content = proxmox_sys::fs::file_read_optional_string(user_config())?.unwrap_or_default();

    let digest = ConfigDigest::from_slice(content.as_bytes());
    let mut data = get_or_init_config().parse(user_config(), &content)?;

    access_conf().init_user_config(&mut data)?;

    Ok((data, digest))
}

pub fn cached_config() -> Result<Arc<SectionConfigData>, Error> {
    struct ConfigCache {
        data: Option<Arc<SectionConfigData>>,
        last_mtime: i64,
        last_mtime_nsec: i64,
    }

    static CACHED_CONFIG: OnceLock<RwLock<ConfigCache>> = OnceLock::new();
    let cached_config = CACHED_CONFIG.get_or_init(|| {
        RwLock::new(ConfigCache {
            data: None,
            last_mtime: 0,
            last_mtime_nsec: 0,
        })
    });

    let stat = match nix::sys::stat::stat(&user_config()) {
        Ok(stat) => Some(stat),
        Err(nix::errno::Errno::ENOENT) => None,
        Err(err) => bail!("unable to stat '{}' - {err}", user_config().display()),
    };

    {
        // limit scope
        let cache = cached_config.read().unwrap();
        if let Some(ref config) = cache.data {
            if let Some(stat) = stat {
                if stat.st_mtime == cache.last_mtime && stat.st_mtime_nsec == cache.last_mtime_nsec
                {
                    return Ok(config.clone());
                }
            } else if cache.last_mtime == 0 && cache.last_mtime_nsec == 0 {
                return Ok(config.clone());
            }
        }
    }

    let (config, _digest) = config()?;
    let config = Arc::new(config);

    let mut cache = cached_config.write().unwrap();
    if let Some(stat) = stat {
        cache.last_mtime = stat.st_mtime;
        cache.last_mtime_nsec = stat.st_mtime_nsec;
    }
    cache.data = Some(config.clone());

    Ok(config)
}

pub fn save_config(config: &SectionConfigData) -> Result<(), Error> {
    let config_file = user_config();
    let raw = get_or_init_config().write(&config_file, config)?;
    replace_privileged_config(config_file, raw.as_bytes())?;

    // increase cache generation so we reload it next time we access it
    access_conf().increment_cache_generation()?;

    Ok(())
}

/// Only exposed for testing
#[doc(hidden)]
pub fn test_cfg_from_str(raw: &str) -> Result<(SectionConfigData, [u8; 32]), Error> {
    let cfg = get_or_init_config();
    let parsed = cfg.parse("test_user_cfg", raw)?;

    Ok((parsed, [0; 32]))
}

// shell completion helper
pub fn complete_userid(_arg: &str, _param: &HashMap<String, String>) -> Vec<String> {
    match config() {
        Ok((data, _digest)) => data
            .sections
            .iter()
            .filter_map(|(id, (section_type, _))| {
                if section_type == "user" {
                    Some(id.to_string())
                } else {
                    None
                }
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

// shell completion helper
pub fn complete_authid(_arg: &str, _param: &HashMap<String, String>) -> Vec<String> {
    match config() {
        Ok((data, _digest)) => data.sections.keys().map(|id| id.to_string()).collect(),
        Err(_) => vec![],
    }
}

// shell completion helper
pub fn complete_token_name(_arg: &str, param: &HashMap<String, String>) -> Vec<String> {
    let data = match config() {
        Ok((data, _digest)) => data,
        Err(_) => return Vec::new(),
    };

    match param.get("userid") {
        Some(userid) => {
            let user = data.lookup::<User>("user", userid);
            let tokens = data.convert_to_typed_array("token");
            match (user, tokens) {
                (Ok(_), Ok(tokens)) => tokens
                    .into_iter()
                    .filter_map(|token: ApiToken| {
                        let tokenid = token.tokenid;
                        if tokenid.is_token() && tokenid.user() == userid {
                            Some(tokenid.tokenname().unwrap().as_str().to_string())
                        } else {
                            None
                        }
                    })
                    .collect(),
                _ => vec![],
            }
        }
        None => vec![],
    }
}
