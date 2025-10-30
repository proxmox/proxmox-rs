use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

use anyhow::{bail, Error};

#[allow(clippy::manual_range_contains)]
fn parse_hex_digit(d: u8) -> Result<u8, Error> {
    if d >= b'0' && d <= b'9' {
        return Ok(d - b'0');
    }
    if d >= b'A' && d <= b'F' {
        return Ok(d - b'A' + 10);
    }
    if d >= b'a' && d <= b'f' {
        return Ok(d - b'a' + 10);
    }
    bail!("got invalid hex digit");
}

/// Escape strings for usage in systemd unit names
#[deprecated = "use proxmox_systemd::escape_unit"]
pub fn escape_unit<P: AsRef<[u8]>>(unit: P, is_path: bool) -> String {
    escape_unit_bytes(unit.as_ref(), is_path)
}

fn escape_unit_bytes(mut unit: &[u8], is_path: bool) -> String {
    if is_path {
        while !unit.is_empty() && unit[0] == b'/' {
            unit = &unit[1..];
        }

        if unit.is_empty() {
            return String::from("-");
        }
    }

    let mut escaped = String::new();

    for (i, c) in unit.iter().enumerate() {
        if *c == b'/' {
            escaped.push('-');
            continue;
        }
        if (i == 0 && *c == b'.')
            || !(*c == b'_'
                || *c == b'.'
                || (*c >= b'0' && *c <= b'9')
                || (*c >= b'A' && *c <= b'Z')
                || (*c >= b'a' && *c <= b'z'))
        {
            use std::fmt::Write as _;
            let _ = write!(escaped, "\\x{c:02x}");
        } else {
            escaped.push(*c as char);
        }
    }
    escaped
}

/// Unescape strings used in systemd unit names
#[deprecated = "use proxmox_systemd::unescape_unit"]
pub fn unescape_unit(text: &str) -> Result<String, Error> {
    Ok(String::from_utf8(unescape_unit_do(text)?)?)
}

/// Unescape strings used in systemd unit names
#[deprecated = "use proxmox_systemd::unescape_unit_path"]
pub fn unescape_unit_path(text: &str) -> Result<PathBuf, Error> {
    Ok(OsString::from_vec(unescape_unit_do(text)?).into())
}

/// Unescape strings used in systemd unit names
fn unescape_unit_do(text: &str) -> Result<Vec<u8>, Error> {
    let mut i = text.as_bytes();

    let mut data: Vec<u8> = Vec::new();

    loop {
        if i.is_empty() {
            break;
        }
        let next = i[0];
        if next == b'\\' {
            if i.len() < 4 {
                bail!("short input");
            }
            if i[1] != b'x' {
                bail!("unkwnown escape sequence");
            }
            let h1 = parse_hex_digit(i[2])?;
            let h0 = parse_hex_digit(i[3])?;
            data.push((h1 << 4) | h0);
            i = &i[4..]
        } else if next == b'-' {
            data.push(b'/');
            i = &i[1..]
        } else {
            data.push(next);
            i = &i[1..]
        }
    }

    Ok(data)
}
