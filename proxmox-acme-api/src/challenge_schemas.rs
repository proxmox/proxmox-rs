//! Read DNS Challenge schemas.
//!
//! Those schemas are provided by debian  package "libproxmox-acme-plugins".

use std::sync::{Arc, LazyLock, Mutex};
use std::time::SystemTime;

use anyhow::Error;
use serde::Serialize;
use serde_json::Value;

use proxmox_sys::fs::file_read_string;

use crate::types::AcmeChallengeSchema;

const ACME_DNS_SCHEMA_FN: &str = "/usr/share/proxmox-acme/dns-challenge-schema.json";

/// Wrapper for efficient Arc use when returning the ACME challenge-plugin schema for serializing.
pub struct ChallengeSchemaWrapper {
    inner: Arc<Vec<AcmeChallengeSchema>>,
}

impl Serialize for ChallengeSchemaWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.inner.serialize(serializer)
    }
}

fn load_dns_challenge_schema() -> Result<Vec<AcmeChallengeSchema>, Error> {
    let raw = file_read_string(ACME_DNS_SCHEMA_FN)?;
    let schemas: serde_json::Map<String, Value> = serde_json::from_str(&raw)?;

    Ok(schemas
        .iter()
        .map(|(id, schema)| AcmeChallengeSchema {
            id: id.to_owned(),
            name: schema
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(id)
                .to_owned(),
            ty: "dns".into(),
            schema: schema.to_owned(),
        })
        .collect())
}

pub fn get_cached_challenge_schemas() -> Result<ChallengeSchemaWrapper, Error> {
    static CACHE: LazyLock<Mutex<Option<(Arc<Vec<AcmeChallengeSchema>>, SystemTime)>>> =
        LazyLock::new(|| Mutex::new(None));

    // the actual loading code
    let mut last = CACHE.lock().unwrap();

    let actual_mtime = std::fs::metadata(ACME_DNS_SCHEMA_FN)?.modified()?;

    let schema = match &*last {
        Some((schema, cached_mtime)) if *cached_mtime >= actual_mtime => schema.clone(),
        _ => {
            let new_schema = Arc::new(load_dns_challenge_schema()?);
            *last = Some((Arc::clone(&new_schema), actual_mtime));
            new_schema
        }
    };

    Ok(ChallengeSchemaWrapper { inner: schema })
}
