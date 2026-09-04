use proxmox_schema::StringSchema;
use proxmox_schema::format::{
    DocumentationFormat, ParameterDisplayStyle, get_property_description_with_conditions,
};

#[test]
fn property_conditions_are_separated() {
    let text = get_property_description_with_conditions(
        "payload",
        &StringSchema::new("The payload.").schema(),
        ParameterDisplayStyle::Arg,
        DocumentationFormat::Full,
        &[("kind", "a"), ("inner", "b")],
    );
    assert!(text.contains("(if kind=\"a\", inner=\"b\")"), "{text}");
}
