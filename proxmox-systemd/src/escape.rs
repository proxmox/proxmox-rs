use std::error::Error as StdError;
use std::ffi::OsString;
use std::fmt;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

/// Escape strings for usage in systemd unit names
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

#[derive(Debug)]
pub enum UnescapeError {
    Msg(&'static str),
    Utf8Error(std::string::FromUtf8Error),
}

impl StdError for UnescapeError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Utf8Error(e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for UnescapeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Msg(err) => f.write_str(err),
            Self::Utf8Error(err) => fmt::Display::fmt(err, f),
        }
    }
}

/// Unescape strings used in systemd unit names
pub fn unescape_unit(text: &str) -> Result<String, UnescapeError> {
    String::from_utf8(unescape_unit_do(text)?).map_err(UnescapeError::Utf8Error)
}

/// Unescape strings used in systemd unit names
pub fn unescape_unit_path(text: &str) -> Result<PathBuf, UnescapeError> {
    Ok(OsString::from_vec(unescape_unit_do(text)?).into())
}

/// Unescape strings used in systemd unit names
fn unescape_unit_do(text: &str) -> Result<Vec<u8>, UnescapeError> {
    let mut i = text.as_bytes();

    let mut data: Vec<u8> = Vec::new();

    loop {
        if i.is_empty() {
            break;
        }
        let next = i[0];
        if next == b'\\' {
            if i.len() < 4 {
                return Err(UnescapeError::Msg("short input"));
            }
            if i[1] != b'x' {
                return Err(UnescapeError::Msg("unknown escape sequence"));
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

fn parse_hex_digit(d: u8) -> Result<u8, UnescapeError> {
    if d.is_ascii_digit() {
        return Ok(d - b'0');
    }
    if (b'A'..=b'F').contains(&d) {
        return Ok(d - b'A' + 10);
    }
    if (b'a'..=b'f').contains(&d) {
        return Ok(d - b'a' + 10);
    }
    Err(UnescapeError::Msg("invalid hex digit"))
}
