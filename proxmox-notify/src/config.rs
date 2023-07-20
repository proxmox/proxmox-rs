use lazy_static::lazy_static;
use proxmox_schema::{ApiType, ObjectSchema};
use proxmox_section_config::{SectionConfig, SectionConfigData, SectionConfigPlugin};

use crate::schema::BACKEND_NAME_SCHEMA;
use crate::Error;

lazy_static! {
    pub static ref CONFIG: SectionConfig = config_init();
    pub static ref PRIVATE_CONFIG: SectionConfig = private_config_init();
}

fn config_init() -> SectionConfig {
    let mut config = SectionConfig::new(&BACKEND_NAME_SCHEMA);

    #[cfg(feature = "sendmail")]
    {
        use crate::endpoints::sendmail::{SendmailConfig, SENDMAIL_TYPENAME};

        const SENDMAIL_SCHEMA: &ObjectSchema = SendmailConfig::API_SCHEMA.unwrap_object_schema();
        config.register_plugin(SectionConfigPlugin::new(
            SENDMAIL_TYPENAME.to_string(),
            Some(String::from("name")),
            SENDMAIL_SCHEMA,
        ));
    }

    config
}

fn private_config_init() -> SectionConfig {
    let mut config = SectionConfig::new(&BACKEND_NAME_SCHEMA);

    config
}

pub fn config(raw_config: &str) -> Result<(SectionConfigData, [u8; 32]), Error> {
    let digest = openssl::sha::sha256(raw_config.as_bytes());
    let data = CONFIG
        .parse("notifications.cfg", raw_config)
        .map_err(|err| Error::ConfigDeserialization(err.into()))?;
    Ok((data, digest))
}

pub fn private_config(raw_config: &str) -> Result<(SectionConfigData, [u8; 32]), Error> {
    let digest = openssl::sha::sha256(raw_config.as_bytes());
    let data = PRIVATE_CONFIG
        .parse("priv/notifications.cfg", raw_config)
        .map_err(|err| Error::ConfigDeserialization(err.into()))?;
    Ok((data, digest))
}

pub fn write(config: &SectionConfigData) -> Result<String, Error> {
    CONFIG
        .write("notifications.cfg", config)
        .map_err(|err| Error::ConfigSerialization(err.into()))
}

pub fn write_private(config: &SectionConfigData) -> Result<String, Error> {
    PRIVATE_CONFIG
        .write("priv/notifications.cfg", config)
        .map_err(|err| Error::ConfigSerialization(err.into()))
}
