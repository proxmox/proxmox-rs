use anyhow::{Error, bail};
use serde::{Deserialize, Serialize};

#[cfg(feature = "enum-fallback")]
use proxmox_fixed_string::FixedString;
use proxmox_schema::*;

const_regex! {
    pub ZPOOL_NAME_REGEX = r"^[a-zA-Z][a-z0-9A-Z\-_.:]+$";
    ZFS_RECORD_SIZE_REGEX = r"^(?i)([1-9][0-9]*)([km])?$";
}

pub const ZFS_ASHIFT_SCHEMA: Schema = IntegerSchema::new("Pool sector size exponent.")
    .minimum(9)
    .maximum(16)
    .default(12)
    .schema();

pub const ZPOOL_NAME_SCHEMA: Schema = StringSchema::new("ZFS Pool Name")
    .format(&ApiStringFormat::Pattern(&ZPOOL_NAME_REGEX))
    .schema();

fn verify_zfs_recordsize(value: &str) -> Result<(), Error> {
    let Some(caps) = ZFS_RECORD_SIZE_REGEX.captures(value) else {
        bail!("Invalid format. Use numbers with optional k or m suffix (e.g., 16k).")
    };

    let range_error = "Value must be a power of 2 between 512 and 16m";

    let Ok(number) = caps[1].parse::<u32>() else {
        bail!(range_error);
    };

    let multiplier = match caps.get(2).map(|v| v.as_str()) {
        Some(suffix) if suffix.eq_ignore_ascii_case("k") => 1024,
        Some(suffix) if suffix.eq_ignore_ascii_case("m") => 1024 * 1024,
        _ => 1,
    };

    let bytes = number
        .checked_mul(multiplier)
        .filter(|&b| b.is_power_of_two() && (512..=16 * 1024 * 1024).contains(&b));

    if bytes.is_none() {
        bail!(range_error);
    }

    Ok(())
}

pub const ZFS_RECORD_SIZE_SCHEMA: Schema =
    StringSchema::new("ZFS Recordsize. Use numbers with optional k or m suffix (e.g., 128k).")
        .format(&ApiStringFormat::VerifyFn(verify_zfs_recordsize))
        .schema();

#[api(default: "On")]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// The ZFS compression algorithm to use.
pub enum ZfsCompressionType {
    /// Gnu Zip
    Gzip,
    /// LZ4
    Lz4,
    /// LZJB
    Lzjb,
    /// ZLE
    Zle,
    /// ZStd
    ZStd,
    /// Enable compression using the default algorithm.
    On,
    /// Disable compression.
    Off,
    #[cfg(feature = "enum-fallback")]
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}

#[api()]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// The ZFS RAID level to use.
pub enum ZfsRaidLevel {
    /// Single Disk
    Single,
    /// Mirror
    Mirror,
    /// Raid10
    Raid10,
    /// RaidZ
    RaidZ,
    /// RaidZ2
    RaidZ2,
    /// RaidZ3
    RaidZ3,
    #[cfg(feature = "enum-fallback")]
    #[serde(untagged)]
    UnknownEnumValue(FixedString),
}

#[api()]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// zpool list item
pub struct ZpoolListItem {
    /// zpool name
    pub name: String,
    /// Health
    pub health: String,
    /// Total size
    pub size: u64,
    /// Used size
    pub alloc: u64,
    /// Free space
    pub free: u64,
    /// ZFS fragnentation level
    pub frag: u64,
    /// ZFS deduplication ratio
    pub dedup: f64,
}

#[cfg(test)]
mod tests {

    use super::verify_zfs_recordsize;

    #[test]
    fn test_zfs_record_size_verify_fn() {
        let format_error = "Invalid format. Use numbers with optional k or m suffix (e.g., 16k).";
        let range_error = "Value must be a power of 2 between 512 and 16m";

        let valid_inputs = ["512", "1024", "16m", "16384k", "16777216"];
        for input in valid_inputs {
            assert!(verify_zfs_recordsize(input).is_ok());
        }

        let range_error_inputs = ["31m", "256", "31m", "32m", "4294967297"];
        for input in range_error_inputs {
            assert_eq!(
                verify_zfs_recordsize(input).unwrap_err().to_string(),
                range_error
            );
        }

        let format_error_inputs = ["0", "k12", "16g", "1.5m", "-512", "0512"];
        for input in format_error_inputs {
            assert_eq!(
                verify_zfs_recordsize(input).unwrap_err().to_string(),
                format_error
            );
        }
    }
}
