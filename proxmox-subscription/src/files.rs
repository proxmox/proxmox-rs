use std::path::Path;

use anyhow::{format_err, Error};
use proxmox_sys::fs::{replace_file, CreateOptions};

use crate::{
    subscription_info::{md5sum, SHARED_KEY_DATA},
    SubscriptionInfo, SubscriptionStatus,
};

pub const DEFAULT_SIGNING_KEY: &str = "/usr/share/keyrings/proxmox-offline-signing-key.pub";

fn parse_subscription_file(raw: &str) -> Result<Option<SubscriptionInfo>, Error> {
    let mut cfg = raw.lines();

    // first line is key in plain
    let key = if let Some(key) = cfg.next() {
        key
    } else {
        return Ok(None);
    };
    // second line is checksum of encoded data
    let checksum = if let Some(csum) = cfg.next() {
        base64::decode(csum)?
    } else {
        return Ok(None);
    };

    // TODO convert to simple collect with PVE 8.0
    let encoded_with_newlines = cfg.fold(String::new(), |mut s, line| {
        s.push_str(line);
        s.push('\n');
        s
    });
    let mut encoded = encoded_with_newlines.clone();
    encoded.retain(|c| c != '\n');

    let decoded = base64::decode(&encoded)?;
    let decoded = std::str::from_utf8(&decoded)?;

    let info: SubscriptionInfo = serde_json::from_str(decoded)?;

    let calc_csum = |encoded: &str| {
        let csum = format!(
            "{}{}{}",
            info.checktime.unwrap_or(0),
            encoded,
            SHARED_KEY_DATA,
        );
        md5sum(csum.as_bytes())
    };

    // TODO drop PVE compat csum with PVE 8.0
    let pve_csum = calc_csum(&encoded_with_newlines)?;
    let pbs_csum = calc_csum(&encoded)?;
    if checksum != pbs_csum.as_ref() && checksum != pve_csum.as_ref() {
        return Ok(Some(SubscriptionInfo {
            status: SubscriptionStatus::Invalid,
            message: Some("checksum mismatch".to_string()),
            ..info
        }));
    }

    match info.key {
        Some(ref info_key) if info_key != key => {
            return Ok(Some(SubscriptionInfo {
                status: SubscriptionStatus::Invalid,
                message: Some("subscription key mismatch".to_string()),
                ..info
            }))
        }
        _ => {}
    }

    Ok(Some(info))
}

/// Reads in subscription information and does a basic integrity verification.
///
/// The expected format consists of three lines:
/// - subscription key
/// - checksum of encoded data
/// - encoded data
///
/// Legacy variants of this format as used by older versions of PVE/PMG are supported.
pub fn read_subscription<P: AsRef<Path>>(
    path: P,
    signature_keys: &[P],
) -> Result<Option<SubscriptionInfo>, Error> {
    match proxmox_sys::fs::file_read_optional_string(path)? {
        Some(raw) => {
            let mut info = parse_subscription_file(&raw)?;
            if let Some(info) = info.as_mut() {
                info.check_signature(signature_keys);
                if info.status == SubscriptionStatus::Active {
                    // these will set `status` to INVALID if checks fail!
                    info.check_server_id();
                    info.check_age(false);
                }
            };

            Ok(info)
        }
        None => Ok(None),
    }
}

/// Writes out subscription status in the format parsed by [`read_subscription`].
pub fn write_subscription<P: AsRef<Path>>(
    path: P,
    file_opts: CreateOptions,
    info: &SubscriptionInfo,
) -> Result<(), Error> {
    let raw = if info.key.is_none() || info.checktime.is_none() {
        String::new()
    } else if let SubscriptionStatus::New = info.status {
        format!("{}\n", info.key.as_ref().unwrap())
    } else {
        let encoded = base64::encode(serde_json::to_string(&info)?);
        let csum = format!(
            "{}{}{}",
            info.checktime.unwrap_or(0),
            encoded,
            SHARED_KEY_DATA
        );
        let csum = base64::encode(md5sum(csum.as_bytes())?);
        format!("{}\n{}\n{}\n", info.key.as_ref().unwrap(), csum, encoded)
    };

    replace_file(path, raw.as_bytes(), file_opts, true)?;

    Ok(())
}

/// Deletes the subscription info file.
pub fn delete_subscription<P: AsRef<Path>>(path: P) -> Result<(), Error> {
    std::fs::remove_file(path)?;

    Ok(())
}

/// Updates apt authentication config for repo access.
pub fn update_apt_auth<P: AsRef<Path>>(
    path: P,
    file_opts: CreateOptions,
    url: &str,
    key: Option<String>,
    password: Option<String>,
) -> Result<(), Error> {
    match (key, password) {
        (Some(key), Some(password)) => {
            let conf = format!("machine {url}\n login {}\n password {}\n", key, password,);

            // we use a namespaced .conf file, so just overwrite..
            replace_file(path, conf.as_bytes(), file_opts, true)
                .map_err(|e| format_err!("Error saving apt auth config - {}", e))?;
        }
        _ => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()), // ignore not existing
            Err(err) => Err(err),
        }
        .map_err(|e| format_err!("Error clearing apt auth config - {}", e))?,
    }
    Ok(())
}

#[test]
fn test_pve_compat() {
    // generated with PVE::Subscription::write_subscription() based on a real test subscription
    let content = "\
        pve4t-123456789a\nNx5qaBSAwkhF/o39/zPAeA\neyJrZXkiOiJwdmU0dC0xMjM0NTY3ODlhIiwibmV4dGR1ZWRhd\
        GUiOiIwMDAwLTAwLTAwIiwic3Rh\ndHVzIjoiQWN0aXZlIiwidmFsaWRkaXJlY3RvcnkiOiI4MzAwMDAwMDAxMjM0NT\
        Y3ODlBQkNERUYw\nMDAwMDA0MiIsImNoZWNrdGltZSI6MTYwMDAwMDAwMCwicHJvZHVjdG5hbWUiOiJQcm94bW94IFZ\
        F\nIEZyZWUgVHJpYWwgU3Vic2NyaXB0aW9uIDEyIE1vbnRocyAoNCBDUFVzKSIsInJlZ2RhdGUiOiIy\nMDIyLTA0LT\
        A3IDAwOjAwOjAwIn0=";

    let expected = SubscriptionInfo {
        status: SubscriptionStatus::Active,
        serverid: Some("830000000123456789ABCDEF00000042".to_string()),
        checktime: Some(1600000000),
        key: Some("pve4t-123456789a".to_string()),
        message: None,
        productname: Some("Proxmox VE Free Trial Subscription 12 Months (4 CPUs)".to_string()),
        regdate: Some("2022-04-07 00:00:00".to_string()),
        nextduedate: Some("0000-00-00".to_string()),
        url: None,
        signature: None,
    };

    let parsed = parse_subscription_file(content);
    assert!(parsed.is_ok());
    let parsed = parsed.unwrap();
    assert!(parsed.is_some());
    let parsed = parsed.unwrap();

    assert_eq!(parsed, expected);
}

#[test]
fn test_pbs_compat() {
    let content = "\
        pbst-123456789a\n//6dnM9V6nNmSh2GbQfZDA==\neyJzdGF0dXMiOiJhY3RpdmUiLCJzZXJ2ZXJpZCI6IjgzMDAw\
        MDAwMDEyMzQ1Njc4OUFCQ0RFRjAwMDAwMDQyIiwiY2hlY2t0aW1lIjoxNjAwMDAwMDAwLCJrZXkiOiJwYnN0LTEyMzQ\
        1Njc4OWEiLCJwcm9kdWN0bmFtZSI6IlByb3htb3ggQmFja3VwIFNlcnZlciBUZXN0IFN1YnNjcmlwdGlvbiAtMSB5ZW\
        FyIiwicmVnZGF0ZSI6IjIwMjAtMDktMTkgMDA6MDA6MDAiLCJuZXh0ZHVlZGF0ZSI6IjIwMjEtMDktMTkiLCJ1cmwiO\
        iJodHRwczovL3d3dy5wcm94bW94LmNvbS9lbi9wcm94bW94LWJhY2t1cC1zZXJ2ZXIvcHJpY2luZyJ9\n";

    let expected = SubscriptionInfo {
        key: Some("pbst-123456789a".to_string()),
        serverid: Some("830000000123456789ABCDEF00000042".to_string()),
        status: SubscriptionStatus::Active,
        checktime: Some(1600000000),
        url: Some("https://www.proxmox.com/en/proxmox-backup-server/pricing".into()),
        message: None,
        nextduedate: Some("2021-09-19".into()),
        regdate: Some("2020-09-19 00:00:00".into()),
        productname: Some("Proxmox Backup Server Test Subscription -1 year".into()),
        signature: None,
    };

    let parsed = parse_subscription_file(content);
    assert!(parsed.is_ok());
    let parsed = parsed.unwrap();
    assert!(parsed.is_some());
    let parsed = parsed.unwrap();

    assert_eq!(parsed, expected);
}
