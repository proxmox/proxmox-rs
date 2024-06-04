
struct ProxmoxProductConfig {
    // Configuration file owner.
    api_user: nix::unistd::User,
}

static mut PRODUCT_CONFIG: Option<ProxmoxProductConfig> = None;

/// Initialize the global product configuration.
pub fn init(api_user: nix::unistd::User) {
    unsafe {
        PRODUCT_CONFIG = Some(ProxmoxProductConfig {
            api_user,
        });
    }
}

/// Returns the global api_user set with [init].
///
/// # Panics
///
/// Panics if [init] wasn't called before.
pub fn get_api_user() -> &'static nix::unistd::User {
    unsafe {
        &PRODUCT_CONFIG
            .as_ref()
            .expect("ProxmoxProductConfig is not initialized!")
            .api_user
    }
}
