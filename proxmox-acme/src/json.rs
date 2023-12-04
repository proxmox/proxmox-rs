use openssl::hash::Hasher;
use serde_json::Value;

use crate::Error;

pub fn to_hash_canonical(value: &Value, output: &mut Hasher) -> Result<(), Error> {
    match value {
        Value::Null | Value::String(_) | Value::Number(_) | Value::Bool(_) => {
            serde_json::to_writer(output, &value)?;
        }
        Value::Array(list) => {
            output.update(b"[")?;
            let mut iter = list.iter();
            if let Some(item) = iter.next() {
                to_hash_canonical(item, output)?;
                for item in iter {
                    output.update(b",")?;
                    to_hash_canonical(item, output)?;
                }
            }
            output.update(b"]")?;
        }
        Value::Object(map) => {
            output.update(b"{")?;
            let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
            keys.sort_unstable();
            let mut iter = keys.into_iter();
            if let Some(key) = iter.next() {
                serde_json::to_writer(&mut *output, &key)?;
                output.update(b":")?;
                to_hash_canonical(&map[key], output)?;
                for key in iter {
                    output.update(b",")?;
                    serde_json::to_writer(&mut *output, &key)?;
                    output.update(b":")?;
                    to_hash_canonical(&map[key], output)?;
                }
            }
            output.update(b"}")?;
        }
    }
    Ok(())
}
