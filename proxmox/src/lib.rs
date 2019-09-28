pub use proxmox_sys as sys;
pub use proxmox_tools as tools;

// Both `proxmox_api` and the 2 macros from `proxmox_api_macro` should be
// exposed via `proxmox::api`.
pub mod api {
    pub use proxmox_api::*;
    #[cfg(feature = "api-macro")]
    pub use proxmox_api_macro::{api, router};
}
