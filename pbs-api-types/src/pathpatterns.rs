use proxmox_schema::{ApiStringFormat, ApiType, Schema, StringSchema, const_regex};

use serde::{Deserialize, Serialize};

const_regex! {
     pub PATH_PATTERN_REGEX = "^.+[^\\\\]$";
}

pub const PATH_PATTERN_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&PATH_PATTERN_REGEX);

pub const PATH_PATTERN_SCHEMA: Schema =
    StringSchema::new("Path or match pattern for matching filenames.")
        .format(&PATH_PATTERN_FORMAT)
        .schema();

#[derive(Default, Deserialize, Serialize)]
/// Path or path pattern for filename matching
pub struct PathPattern {
    pattern: String,
}

impl ApiType for PathPattern {
    const API_SCHEMA: Schema = PATH_PATTERN_SCHEMA;
}

impl AsRef<[u8]> for PathPattern {
    fn as_ref(&self) -> &[u8] {
        self.pattern.as_bytes()
    }
}
