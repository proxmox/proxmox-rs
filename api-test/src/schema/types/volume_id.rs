//! A 'VolumeId' is a storage + volume combination.

use failure::{format_err, Error};

use proxmox::api::api;

#[api({
    serialize_as_string: true,
    cli: FromStr,
    description: "A volume ID consisting of a storage name and a volume name",
    fields: {
        storage: {
            description: "A storage name",
            pattern: r#"^[a-z][a-z0-9\-_.]*[a-z0-9]$"#,
        },
        volume: "A volume name",
    },
})]
#[derive(Clone)]
pub struct VolumeId {
    storage: String,
    volume: String,
}

impl std::fmt::Display for VolumeId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}:{}", self.storage, self.volume)
    }
}

impl std::str::FromStr for VolumeId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(2, ':');

        let this = Self {
            storage: parts
                .next()
                .ok_or_else(|| format_err!("not a volume id: {}", s))?
                .to_string(),
            volume: parts
                .next()
                .ok_or_else(|| format_err!("not a volume id: {}", s))?
                .to_string(),
        };
        assert!(parts.next().is_none());

        proxmox::api::ApiType::verify(&this)?;

        Ok(this)
    }
}
