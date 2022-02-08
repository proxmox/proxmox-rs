use std::fmt::Write;

use anyhow::{bail, Error};
use serde_json::Value;

use crate::MetricsData;

pub(crate) fn format_influxdb_line(data: &MetricsData) -> Result<String, Error> {
    if !data.values.is_object() {
        bail!("invalid data");
    }

    let mut line = escape_measurement(&data.measurement);

    for (key, value) in &data.tags {
        write!(line, ",{}={}", escape_key(key), escape_key(value))?;
    }

    line.push(' ');

    let mut first = true;
    for (key, value) in data.values.as_object().unwrap().iter() {
        match value {
            Value::Object(_) => bail!("objects not supported"),
            Value::Array(_) => bail!("arrays not supported"),
            _ => {}
        }
        if !first {
            line.push(',');
        }
        first = false;
        write!(line, "{}={}", escape_key(key), value)?;
    }

    // nanosecond precision
    writeln!(line, " {}", data.ctime * 1_000_000_000)?;

    Ok(line)
}

fn escape_key(key: &str) -> String {
    let key = key.replace(',', "\\,");
    let key = key.replace('=', "\\=");
    key.replace(' ', "\\ ")
}

fn escape_measurement(measurement: &str) -> String {
    let measurement = measurement.replace(',', "\\,");
    measurement.replace(' ', "\\ ")
}
