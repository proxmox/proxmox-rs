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
    let mut js_date = js_date
        .to_iso_string()
        .as_string()
        .ok_or_else(|| format_err!("to_iso_string did not return a string"))?;

    match js_date.len() {
        len if len < 24 => bail!("invalid length {len} for rfc3339 string"),
        len => {
            js_date.replace_range((len - 5).., "Z"); // replace .xxxZ with Z
            Ok(js_date)
        }
    }
}

/// Convert Unix epoch into RFC3339 local time with TZ
pub fn epoch_to_rfc3339(epoch: i64) -> Result<String, Error> {
    let js_date = js_sys::Date::new_0();
    js_date.set_time((epoch as f64) * 1000.0);

    let y = js_date.get_full_year();
    let m = js_date.get_month() + 1;
    let d = js_date.get_date();
    let h = js_date.get_hours();
    let min = js_date.get_minutes();
    let s = js_date.get_seconds();

    let offset = -js_date.get_timezone_offset() as i64;

    let offset = if offset == 0 {
        "Z".to_string()
    } else {
        let offset_hour = (offset / 60).abs();
        let offset_minute = (offset % 60).abs();
        let sign = if offset > 0 { "+" } else { "-" };
        format!("{sign}{offset_hour:0>2}:{offset_minute:0>2}")
    };

    Ok(format!(
        "{y:0>4}-{m:0>2}-{d:0>2}T{h:0>2}:{min:0>2}:{s:0>2}{offset}"
    ))
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
