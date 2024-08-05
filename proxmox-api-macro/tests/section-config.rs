use serde::{Deserialize, Serialize};

use proxmox_api_macro::api;
use proxmox_section_config::typed::ApiSectionDataEntry;

#[api]
/// Type A.
#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct TypeA {
    /// The id.
    id: String,

    /// Some name.
    name: String,
}

#[api]
/// Type B.
#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct TypeB {
    /// The id.
    id: String,

    /// An age.
    age: u64,

    /// The internally tagged type.
    #[serde(rename = "the-type")]
    ty: String,
}

#[api(
    "id-property": "id",
    "id-schema": {
        type: String,
        description: "A config ID",
        max_length: 16,
    },
)]
#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Config {
    A(TypeA),
    #[api(type_key = "the-type")]
    B(TypeB),
}

#[test]
fn test_config() {
    let content = "\
        A: the-a\n\
            \tname The Name\n\
        \n\
        B: the-b\n\
            \tage 42\n\
    ";

    let data = Config::parse_section_config("a_test_file.cfg", content)
        .expect("failed to parse test section config");

    assert_eq!(data.len(), 2);
    assert_eq!(
        data["the-a"],
        Config::A(TypeA {
            id: "the-a".to_string(),
            name: "The Name".to_string(),
        })
    );
    assert_eq!(
        data["the-b"],
        Config::B(TypeB {
            id: "the-b".to_string(),
            age: 42,
            ty: "B".to_string(),
        })
    );

    let raw = Config::write_section_config("a_test_output_file.cfg", &data)
        .expect("failed to write out test section config");
    assert_eq!(raw, content);
}

#[api]
/// Type A2.
#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct TypeA2 {
    /// The id.
    id: String,

    /// Some name.
    name: String,

    /// The internally tagged type.
    ty: String,
}

#[api(
    "id-property": "id",
    "id-schema": {
        type: String,
        description: "A config ID",
        max_length: 16,
    },
    "type-key": "ty",
)]
#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ConfigTypeKey {
    A2(TypeA2),
    #[api(type_key = "the-type")]
    B(TypeB),
}

#[test]
fn test_global_type_key() {
    let content = "\
        a2: the-a\n\
            \tname The Name\n\
        \n\
        b: the-b\n\
            \tage 42\n\
    ";

    let data = ConfigTypeKey::parse_section_config("a_test_file.cfg", content)
        .expect("failed to parse test section config");

    assert_eq!(data.len(), 2);
    assert_eq!(
        data["the-a"],
        ConfigTypeKey::A2(TypeA2 {
            id: "the-a".to_string(),
            name: "The Name".to_string(),
            ty: "a2".to_string(),
        })
    );
    assert_eq!(
        data["the-b"],
        ConfigTypeKey::B(TypeB {
            id: "the-b".to_string(),
            age: 42,
            ty: "b".to_string(),
        })
    );

    let raw = ConfigTypeKey::write_section_config("a_test_output_file.cfg", &data)
        .expect("failed to write out test section config");
    assert_eq!(raw, content);
}
