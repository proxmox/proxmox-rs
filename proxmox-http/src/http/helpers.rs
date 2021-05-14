use anyhow::Error;

use http::uri::Authority;

// Build a http::uri::Authority ("host:port"), use '[..]' around IPv6 addresses
pub fn build_authority(host: &str, port: u16) -> Result<Authority, Error> {
    let bytes = host.as_bytes();
    let len = bytes.len();
    let authority = if len > 3 && bytes.contains(&b':') && bytes[0] != b'[' && bytes[len-1] != b']' {
        format!("[{}]:{}", host, port).parse()?
    } else {
        format!("{}:{}", host, port).parse()?
    };
    Ok(authority)
}
