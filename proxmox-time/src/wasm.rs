use anyhow::{bail, format_err, Error};

/// Returns Unix Epoch (now)
pub fn epoch_i64() -> i64 {
    epoch_f64() as i64
}

/// Returns Unix Epoch (now) as f64 with subseconds resolution
pub fn epoch_f64() -> f64 {
    js_sys::Date::now() / 1000.0
}

/// Convert Unix epoch into RFC3339 UTC string
pub fn epoch_to_rfc3339_utc(epoch: i64) -> Result<String, Error> {
    let js_date = js_sys::Date::new_0();
    js_date.set_time((epoch as f64) * 1000.0);
    js_date
        .to_iso_string()
        .as_string()
        .ok_or_else(|| format_err!("to_iso_string did not return a string"))
}

/// Convert Unix epoch into RFC3339 local time with TZ
pub fn epoch_to_rfc3339(epoch: i64) -> Result<String, Error> {
    // Note: JS does not provide this, so we need to implement this ourselves.
    // for now, we simply return UTC instead
    epoch_to_rfc3339_utc(epoch)
}

/// Parse RFC3339 into Unix epoch
pub fn parse_rfc3339(input_str: &str) -> Result<i64, Error> {
    // TOTO: This should parse olny RFC3339, but currently also parse
    // other formats
    let time_milli = js_sys::Date::parse(input_str);
    if time_milli.is_nan() {
        bail!("unable to parse RFC3339 time");
    }
    Ok((time_milli / 1000.0) as i64)
}
