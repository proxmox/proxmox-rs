use std::path::{Path, PathBuf};

static mut PRODUCT_CONFIG: Option<ProxmoxProductConfig> = None;

/// Initialize the global product configuration.
pub fn init_product_config(config_dir: &'static str, api_user: nix::unistd::User) {
    unsafe {
        PRODUCT_CONFIG = Some(ProxmoxProductConfig {
            config_dir,
            api_user,
        });
    }
}

/// Returns the global product configuration (see [init_product_config])
pub fn product_config() -> &'static ProxmoxProductConfig {
    unsafe {
        PRODUCT_CONFIG
            .as_ref()
            .expect("ProxmoxProductConfig is not initialized!")
    }
}

pub struct ProxmoxProductConfig {
    /// Path to the main product configuration directory.
    pub(crate) config_dir: &'static str,

    /// Configuration file owner.
    pub(crate) api_user: nix::unistd::User,
}

impl ProxmoxProductConfig {
    /// Returns the absolute path (prefix with 'config_dir')
    pub fn absolute_path(&self, rel_path: &str) -> PathBuf {
        let path = Path::new(self.config_dir);
        path.join(rel_path)
    }
}
