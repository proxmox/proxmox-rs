//! URI Related helpers, such as `build_authority`

use http::uri::{Authority, InvalidUri};

// Build an [`Authority`](http::uri::Authority) from a combination of `host` and `port`, ensuring that
// IPv6 addresses are enclosed in brackets.
pub fn build_authority(host: &str, port: u16) -> Result<Authority, InvalidUri> {
    let bytes = host.as_bytes();
    let len = bytes.len();
    let authority =
        if len > 3 && bytes.contains(&b':') && bytes[0] != b'[' && bytes[len - 1] != b']' {
            format!("[{}]:{}", host, port).parse()?
        } else {
            format!("{}:{}", host, port).parse()?
        };
    Ok(authority)
}
