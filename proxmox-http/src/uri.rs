//! URI Related helpers, such as `build_authority`

use anyhow::bail;
use anyhow::format_err;
use anyhow::Error;
use http::uri::{Authority, InvalidUri};
use serde_json::Value;

// Build an [`Authority`](http::uri::Authority) from a combination of `host` and `port`, ensuring that
// IPv6 addresses are enclosed in brackets.
pub fn build_authority(host: &str, port: u16) -> Result<Authority, InvalidUri> {
    let bytes = host.as_bytes();
    let len = bytes.len();
    let authority =
        if len > 3 && bytes.contains(&b':') && bytes[0] != b'[' && bytes[len - 1] != b']' {
            format!("[{host}]:{port}").parse()?
        } else {
            format!("{host}:{port}").parse()?
        };
    Ok(authority)
}

pub fn json_object_to_query(data: Value) -> Result<String, Error> {
    let mut query = url::form_urlencoded::Serializer::new(String::new());

    let object = data.as_object().ok_or_else(|| {
        format_err!("json_object_to_query: got wrong data type (expected object).")
    })?;

    for (key, value) in object {
        match value {
            Value::Bool(b) => {
                query.append_pair(key, &b.to_string());
            }
            Value::Number(n) => {
                query.append_pair(key, &n.to_string());
            }
            Value::String(s) => {
                query.append_pair(key, s);
            }
            Value::Array(arr) => {
                for element in arr {
                    match element {
                        Value::Bool(b) => {
                            query.append_pair(key, &b.to_string());
                        }
                        Value::Number(n) => {
                            query.append_pair(key, &n.to_string());
                        }
                        Value::String(s) => {
                            query.append_pair(key, s);
                        }
                        _ => bail!(
                            "json_object_to_query: unable to handle complex array data types."
                        ),
                    }
                }
            }
            _ => bail!("json_object_to_query: unable to handle complex data types."),
        }
    }

    Ok(query.finish())
}
