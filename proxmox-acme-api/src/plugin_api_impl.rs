//! ACME plugin configuration API implementation

use anyhow::{bail, format_err, Error};

use serde::Deserialize;
use serde_json::Value;

use proxmox_config_digest::ConfigDigest;
use proxmox_schema::param_bail;

use crate::types::{
    DeletablePluginProperty, DnsPlugin, DnsPluginCore, DnsPluginCoreUpdater, PluginConfig,
};

use proxmox_router::{http_bail, RpcEnvironment};

pub fn list_plugins(rpcenv: &mut dyn RpcEnvironment) -> Result<Vec<PluginConfig>, Error> {
    let (plugins, digest) = super::plugin_config::plugin_config()?;

    rpcenv["digest"] = digest.to_hex().into();
    Ok(plugins
        .iter()
        .map(|(id, (ty, data))| modify_cfg_for_api(id, ty, data))
        .collect())
}

pub fn get_plugin(id: String, rpcenv: &mut dyn RpcEnvironment) -> Result<PluginConfig, Error> {
    let (plugins, digest) = super::plugin_config::plugin_config()?;
    rpcenv["digest"] = digest.to_hex().into();

    match plugins.get(&id) {
        Some((ty, data)) => Ok(modify_cfg_for_api(&id, ty, data)),
        None => http_bail!(NOT_FOUND, "no such plugin"),
    }
}

pub fn add_plugin(r#type: String, core: DnsPluginCore, data: String) -> Result<(), Error> {
    // Currently we only support DNS plugins and the standalone plugin is "fixed":
    if r#type != "dns" {
        param_bail!("type", "invalid ACME plugin type: {:?}", r#type);
    }

    let data = String::from_utf8(base64::decode(data)?)
        .map_err(|_| format_err!("data must be valid UTF-8"))?;

    let id = core.id.clone();

    let _lock = super::plugin_config::lock_plugin_config()?;

    let (mut plugins, _digest) = super::plugin_config::plugin_config()?;
    if plugins.contains_key(&id) {
        param_bail!("id", "ACME plugin ID {:?} already exists", id);
    }

    let plugin = serde_json::to_value(DnsPlugin { core, data })?;

    plugins.insert(id, r#type, plugin);

    super::plugin_config::save_plugin_config(&plugins)?;

    Ok(())
}

pub fn update_plugin(
    id: String,
    update: DnsPluginCoreUpdater,
    data: Option<String>,
    delete: Option<Vec<DeletablePluginProperty>>,
    digest: Option<ConfigDigest>,
) -> Result<(), Error> {
    let data = data
        .as_deref()
        .map(base64::decode)
        .transpose()?
        .map(String::from_utf8)
        .transpose()
        .map_err(|_| format_err!("data must be valid UTF-8"))?;

    let _lock = super::plugin_config::lock_plugin_config()?;

    let (mut plugins, expected_digest) = super::plugin_config::plugin_config()?;

    expected_digest.detect_modification(digest.as_ref())?;

    match plugins.get_mut(&id) {
        Some((ty, ref mut entry)) => {
            if ty != "dns" {
                bail!("cannot update plugin of type {:?}", ty);
            }

            let mut plugin = DnsPlugin::deserialize(&*entry)?;

            if let Some(delete) = delete {
                for delete_prop in delete {
                    match delete_prop {
                        DeletablePluginProperty::ValidationDelay => {
                            plugin.core.validation_delay = None;
                        }
                        DeletablePluginProperty::Disable => {
                            plugin.core.disable = None;
                        }
                    }
                }
            }
            if let Some(data) = data {
                plugin.data = data;
            }
            if let Some(api) = update.api {
                plugin.core.api = api;
            }
            if update.validation_delay.is_some() {
                plugin.core.validation_delay = update.validation_delay;
            }
            if update.disable.is_some() {
                plugin.core.disable = update.disable;
            }

            *entry = serde_json::to_value(plugin)?;
        }
        None => http_bail!(NOT_FOUND, "no such plugin"),
    }

    super::plugin_config::save_plugin_config(&plugins)?;

    Ok(())
}

pub fn delete_plugin(id: String) -> Result<(), Error> {
    let _lock = super::plugin_config::lock_plugin_config()?;

    let (mut plugins, _digest) = super::plugin_config::plugin_config()?;
    if plugins.remove(&id).is_none() {
        http_bail!(NOT_FOUND, "no such plugin");
    }
    super::plugin_config::save_plugin_config(&plugins)?;

    Ok(())
}

// See PMG/PVE's $modify_cfg_for_api sub
fn modify_cfg_for_api(id: &str, ty: &str, data: &Value) -> PluginConfig {
    let mut entry = data.clone();

    let obj = entry.as_object_mut().unwrap();
    obj.remove("id");
    obj.insert("plugin".to_string(), Value::String(id.to_owned()));
    obj.insert("type".to_string(), Value::String(ty.to_owned()));

    // FIXME: This needs to go once the `Updater` is fixed.
    // None of these should be able to fail unless the user changed the files by hand, in which
    // case we leave the unmodified string in the Value for now. This will be handled with an error
    // later.
    if let Some(Value::String(ref mut data)) = obj.get_mut("data") {
        if let Ok(new) = base64::decode_config(&data, base64::URL_SAFE_NO_PAD) {
            if let Ok(utf8) = String::from_utf8(new) {
                *data = utf8;
            }
        }
    }

    // PVE/PMG do this explicitly for ACME plugins...
    // obj.insert("digest".to_string(), Value::String(digest.clone()));

    serde_json::from_value(entry).unwrap_or_else(|_| PluginConfig {
        plugin: "*Error*".to_string(),
        ty: "*Error*".to_string(),
        ..Default::default()
    })
}
