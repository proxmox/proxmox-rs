pub use proxmox_tools as tools;
pub use proxmox_sys as sys;

// Both `proxmox_api` and the 2 macros from `proxmox_api_macro` should be
// exposed via `proxmox::api`.
pub mod api {
    pub use proxmox_api::*;
    pub use proxmox_api_macro::{api, router};
}
