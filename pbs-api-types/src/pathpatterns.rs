use proxmox_schema::{const_regex, ApiStringFormat, ApiType, ArraySchema, Schema, StringSchema};

use serde::{Deserialize, Serialize};

const_regex! {
     pub PATH_PATTERN_REGEX = concat!(r"^.+[^\\]$");
}

pub const PATH_PATTERN_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&PATH_PATTERN_REGEX);

pub const PATH_PATTERN_SCHEMA: Schema =
    StringSchema::new("Path or match pattern for matching filenames.")
        .format(&PATH_PATTERN_FORMAT)
        .schema();

pub const PATH_PATTERN_LIST_SCHEMA: Schema = ArraySchema::new(
    "List of paths or match patterns for matching filenames.",
    &PATH_PATTERN_SCHEMA,
)
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

#[derive(Default, Deserialize, Serialize)]
/// Array of paths and/or path patterns for filename matching
pub struct PathPatterns {
    patterns: Vec<PathPattern>,
}

impl ApiType for PathPatterns {
    const API_SCHEMA: Schema = PATH_PATTERN_LIST_SCHEMA;
}

impl IntoIterator for PathPatterns {
    type Item = PathPattern;
    type IntoIter = std::vec::IntoIter<PathPattern>;

    fn into_iter(self) -> Self::IntoIter {
        self.patterns.into_iter()
    }
}
