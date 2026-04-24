use std::fmt;

use crate::sys::{self, sd_id128_t};

#[derive(Debug, PartialEq, Eq)]
pub enum SystemdId128Error {
    InvalidAppId,
    GenerationError,
}

impl std::error::Error for SystemdId128Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl fmt::Display for SystemdId128Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SystemdId128Error::InvalidAppId => f.write_str("Provided application ID is invalid."),
            SystemdId128Error::GenerationError => {
                f.write_str("Failed to generate machine-id based on application ID.")
            }
        }
    }
}

pub fn get_app_specific_id(app_id: [u8; 16]) -> Result<[u8; 16], SystemdId128Error> {
    let mut res = sd_id128_t { bytes: [0; 16] };

    if app_id.iter().all(|b| *b == 0) {
        return Err(SystemdId128Error::InvalidAppId);
    }
    unsafe {
        sys::sd_id128_get_machine_app_specific(sd_id128_t { bytes: app_id }, &mut res);
    }
    if res.bytes.iter().all(|b| *b == 0) {
        return Err(SystemdId128Error::GenerationError);
    }
    Ok(res.bytes)
}

#[test]
fn test_invalid_app_id() {
    let invalid = [0; 16];
    let res = get_app_specific_id(invalid);
    assert!(res.is_err());
    assert_eq!(res, Err(SystemdId128Error::InvalidAppId));
}

#[test]
fn test_valid_app_id() {
    // no machine-id, no app-specific ID either..
    if !std::path::Path::new("/etc/machine-id").exists() {
        return;
    }

    // UUID generated with `systemd-id128 new` and converted from hex
    let valid = 950247666410175165299169499632875718_u128.to_le_bytes();

    let res = get_app_specific_id(valid);
    assert!(res.is_ok());

    let res2 = get_app_specific_id(valid);
    assert!(res2.is_ok());

    // cannot verify the expected result, since that depends on the machine the test runs on
    // we can verify that two generations using the same machine and app-id give identical results
    assert_eq!(res, res2);
}
