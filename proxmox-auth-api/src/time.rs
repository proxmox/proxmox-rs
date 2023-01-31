use std::time::{SystemTime, UNIX_EPOCH};

/// Unix epoch.
pub fn epoch_i64() -> i64 {
    let now = SystemTime::now();

    if now > UNIX_EPOCH {
        i64::try_from(now.duration_since(UNIX_EPOCH).unwrap().as_secs()).expect("epoch > 64 bit")
    } else {
        -i64::try_from(UNIX_EPOCH.duration_since(now).unwrap().as_secs()).expect("epoch > 64 bit")
    }
}
