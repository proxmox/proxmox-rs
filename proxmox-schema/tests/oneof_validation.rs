use proxmox_schema::*;
use serde_json::{Value, json};

static STRING: Schema = StringSchema::new("A string.").schema();
// Allows additional properties, so that only the type check rejects an unknown variant's payload.
static PAYLOAD: Schema = ObjectSchema::new("Payload.", &[("payload", false, &STRING)])
    .additional_properties(true)
    .schema();
static CHOICE: Schema = OneOfSchema::new(
    "Choice with a plain string type.",
    &("kind", false, &STRING),
    &[("a", &PAYLOAD)],
)
.schema();
static ALL: Schema = AllOfSchema::new("Flattened choice.", &[&CHOICE]).schema();

fn parameter_schema(schema: &'static Schema) -> ParameterSchema {
    match schema {
        Schema::OneOf(schema) => schema.into(),
        Schema::AllOf(schema) => schema.into(),
        _ => panic!("expected a combined schema"),
    }
}

fn parameters(fields: &[(&str, &str)]) -> Vec<(String, String)> {
    fields
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

/// Check property string, parameter and JSON verification.
fn check_all_parsers(schema: &'static Schema, fields: &[(&str, &str)], valid: bool) {
    let input = fields
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(",");
    let value: Value = fields
        .iter()
        .map(|(key, value)| (key.to_string(), json!(value)))
        .collect();

    assert_eq!(de::verify(schema, &input).is_ok(), valid, "{input}");
    assert_eq!(
        parameter_schema(schema)
            .parse_parameter_strings(&parameters(fields), true)
            .is_ok(),
        valid,
        "{input}"
    );
    assert_eq!(schema.verify_json(&value).is_ok(), valid, "{value}");
}

fn paths(errors: ParameterError) -> Vec<String> {
    errors
        .into_inner()
        .into_iter()
        .map(|(path, _)| path)
        .collect()
}

#[test]
fn unknown_types_are_rejected() {
    for schema in [&CHOICE, &ALL] {
        check_all_parsers(schema, &[("kind", "a"), ("payload", "value")], true);
        check_all_parsers(schema, &[("kind", "unknown")], false);
        check_all_parsers(schema, &[("kind", "unknown"), ("payload", "value")], false);

        // partial parameters may lack the type, but must not name an unknown one
        let schema = parameter_schema(schema);
        assert!(schema.parse_parameter_strings(&[], false).is_ok());
        assert!(
            schema
                .parse_parameter_strings(&parameters(&[("kind", "unknown")]), false)
                .is_err()
        );
    }
}

#[test]
fn enum_types_need_a_variant() {
    static TAG: Schema = StringSchema::new("Tag.")
        .format(&ApiStringFormat::Enum(&[
            EnumEntry::new("a", "Supported."),
            EnumEntry::new("unknown", "Without variant."),
        ]))
        .schema();
    static CHOICE: Schema =
        OneOfSchema::new("Choice.", &("kind", false, &TAG), &[("a", &PAYLOAD)]).schema();
    static ALL: Schema = AllOfSchema::new("Flattened choice.", &[&CHOICE]).schema();
    check_all_parsers(&ALL, &[("kind", "unknown")], false);

    // a value outside of the enumeration is reported once
    let error = ALL.verify_json(&json!({"kind": "other"})).unwrap_err();
    assert_eq!(paths(error.downcast().unwrap()), ["kind"]);
}

#[test]
fn nested_types_follow_the_active_variant() {
    static INNER: Schema = OneOfSchema::new(
        "Inner choice.",
        &("inner", false, &STRING),
        &[("nested", &PAYLOAD)],
    )
    .schema();
    static EMPTY: Schema = ObjectSchema::new("Empty.", &[])
        .additional_properties(true)
        .schema();
    static OUTER: Schema = OneOfSchema::new(
        "Outer choice.",
        &("outer", false, &STRING),
        &[("a", &INNER), ("b", &EMPTY)],
    )
    .schema();
    static ALL: Schema = AllOfSchema::new("Flattened outer choice.", &[&OUTER]).schema();
    check_all_parsers(
        &ALL,
        &[("outer", "a"), ("inner", "nested"), ("payload", "value")],
        true,
    );
    check_all_parsers(&ALL, &[("outer", "a"), ("inner", "unknown")], false);
    // variant `b` has no inner choice, `inner` is an ordinary additional property there
    check_all_parsers(&ALL, &[("outer", "b"), ("inner", "unknown")], true);
}

#[test]
fn type_errors_are_reported_with_other_errors() {
    static STRICT: Schema = OneOfSchema::new(
        "Choice without additional properties.",
        &("kind", false, &STRING),
        &[(
            "a",
            &ObjectSchema::new("Payload.", &[("payload", false, &STRING)]).schema(),
        )],
    )
    .schema();
    static OTHER: Schema = OneOfSchema::new(
        "Independent choice.",
        &("other", false, &STRING),
        &[("empty", &ObjectSchema::new("Empty.", &[]).schema())],
    )
    .schema();
    static ALL: Schema = AllOfSchema::new("Independent choices.", &[&STRICT, &OTHER]).schema();

    let fields = [
        ("kind", "unknown"),
        ("other", "unknown"),
        ("payload", "value"),
    ];
    let expected = ["payload", "kind", "other"];

    let error = de::verify(&ALL, "kind=unknown,other=unknown,payload=value").unwrap_err();
    assert_eq!(paths(error.downcast().unwrap()), expected);
    let error = parameter_schema(&ALL)
        .parse_parameter_strings(&parameters(&fields), true)
        .unwrap_err();
    assert_eq!(paths(error), expected);
    let error = ALL
        .verify_json(&json!({"kind": "unknown", "other": "unknown", "payload": "value"}))
        .unwrap_err();
    assert_eq!(paths(error.downcast().unwrap()), expected);
}
