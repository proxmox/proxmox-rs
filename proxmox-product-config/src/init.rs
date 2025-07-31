use std::sync::OnceLock;

#[derive(Debug)]
struct ProxmoxProductConfig {
    api_user: nix::unistd::User,
    priv_user: nix::unistd::User,
}

static PRODUCT_CONFIG: OnceLock<ProxmoxProductConfig> = OnceLock::new();

/// Initialize the global product configuration.
pub fn init(api_user: nix::unistd::User, priv_user: nix::unistd::User) {
    PRODUCT_CONFIG
        .set(ProxmoxProductConfig {
            api_user,
            priv_user,
        })
        .expect("cannot init proxmox product config twice");
}

/// Returns the global api user set with [init].
///
/// # Panics
///
/// Panics if [init] wasn't called before.
pub fn get_api_user() -> &'static nix::unistd::User {
    &PRODUCT_CONFIG
        .get()
        .expect("ProxmoxProductConfig is not initialized!")
        .api_user
}

/// Returns the global privileged user set with [init].
///
/// # Panics
///
/// Panics if [init] wasn't called before.
pub fn get_priv_user() -> &'static nix::unistd::User {
    &PRODUCT_CONFIG
        .get()
        .expect("ProxmoxProductConfig is not initialized!")
        .priv_user
}
