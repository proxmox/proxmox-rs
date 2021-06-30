use once_cell::sync::OnceCell;

static GLOBAL_CONFIG: OnceCell<APTConfig> = OnceCell::new();

/// APT configuration variables.
pub struct APTConfig {
    /// Dir::State
    pub dir_state: String,
    /// Dir::State::Lists
    pub dir_state_lists: String,
}

impl APTConfig {
    /// Create a new configuration overriding the provided values.
    pub fn new(dir_state: Option<&str>, dir_state_lists: Option<&str>) -> Self {
        Self {
            dir_state: dir_state.unwrap_or("/var/lib/apt/").to_string(),
            dir_state_lists: dir_state_lists.unwrap_or("lists/").to_string(),
        }
    }
}

/// Get the configuration.
///
/// Initializes with default values if init() wasn't called before.
pub fn get() -> &'static APTConfig {
    &GLOBAL_CONFIG.get_or_init(|| APTConfig::new(None, None))
}

/// Initialize the configuration.
///
/// Only has an effect if no init() or get() has been called yet.
pub fn init(config: APTConfig) -> &'static APTConfig {
    &GLOBAL_CONFIG.get_or_init(|| config)
}
