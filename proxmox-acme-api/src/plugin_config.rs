//! ACME plugin configuration helpers (SectionConfig implementation)

use anyhow::Error;
use lazy_static::lazy_static;
use serde_json::Value;

use proxmox_schema::{ApiType, Schema};
use proxmox_section_config::{SectionConfig, SectionConfigData, SectionConfigPlugin};
use proxmox_config_digest::ConfigDigest;
use proxmox_product_config::{ApiLockGuard, open_api_lockfile, replace_config};

use crate::types::{DnsPlugin, StandalonePlugin, PLUGIN_ID_SCHEMA};

lazy_static! {
    static ref CONFIG: SectionConfig = init();
}

impl DnsPlugin {
    pub fn decode_data(&self, output: &mut Vec<u8>) -> Result<(), Error> {
        Ok(base64::decode_config_buf(
            &self.data,
            base64::URL_SAFE_NO_PAD,
            output,
        )?)
    }
}

fn init() -> SectionConfig {
    let mut config = SectionConfig::new(&PLUGIN_ID_SCHEMA);

    let standalone_schema = match &StandalonePlugin::API_SCHEMA {
        Schema::Object(schema) => schema,
        _ => unreachable!(),
    };
    let standalone_plugin = SectionConfigPlugin::new(
        "standalone".to_string(),
        Some("id".to_string()),
        standalone_schema,
    );
    config.register_plugin(standalone_plugin);

    let dns_challenge_schema = match DnsPlugin::API_SCHEMA {
        Schema::AllOf(ref schema) => schema,
        _ => unreachable!(),
    };
    let dns_challenge_plugin = SectionConfigPlugin::new(
        "dns".to_string(),
        Some("id".to_string()),
        dns_challenge_schema,
    );
    config.register_plugin(dns_challenge_plugin);

    config
}

pub(crate) fn lock_plugin_config() -> Result<ApiLockGuard, Error> {
    let plugin_cfg_lockfile = crate::plugin_cfg_lockfile();
    open_api_lockfile(plugin_cfg_lockfile, None, true)
}

pub(crate) fn plugin_config() -> Result<(PluginData, ConfigDigest), Error> {
    let plugin_cfg_filename = crate::plugin_cfg_filename();

    let content =
        proxmox_sys::fs::file_read_optional_string(&plugin_cfg_filename)?.unwrap_or_default();

    let digest = ConfigDigest::from_slice(content.as_bytes());
    let mut data = CONFIG.parse(plugin_cfg_filename, &content)?;

    if data.sections.get("standalone").is_none() {
        let standalone = StandalonePlugin::default();
        data.set_data("standalone", "standalone", &standalone)
            .unwrap();
    }

    Ok((PluginData { data }, digest))
}

pub(crate) fn save_plugin_config(config: &PluginData) -> Result<(), Error> {
    let plugin_cfg_filename = crate::plugin_cfg_filename();
    let raw = CONFIG.write(&plugin_cfg_filename, &config.data)?;

    replace_config(plugin_cfg_filename, raw.as_bytes())
}

pub(crate) struct PluginData {
    data: SectionConfigData,
}

// And some convenience helpers.
impl PluginData {
    pub fn remove(&mut self, name: &str) -> Option<(String, Value)> {
        self.data.sections.remove(name)
    }

    pub fn contains_key(&mut self, name: &str) -> bool {
        self.data.sections.contains_key(name)
    }

    pub fn get(&self, name: &str) -> Option<&(String, Value)> {
        self.data.sections.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut (String, Value)> {
        self.data.sections.get_mut(name)
    }

    pub fn insert(&mut self, id: String, ty: String, plugin: Value) {
        self.data.sections.insert(id, (ty, plugin));
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &(String, Value))> + Send {
        self.data.sections.iter()
    }
}
