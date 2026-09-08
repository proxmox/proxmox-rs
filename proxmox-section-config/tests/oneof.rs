use proxmox_schema::*;
use proxmox_section_config::*;
use serde_json::json;

static STRING: Schema = StringSchema::new("A string.").schema();
static INTEGER: Schema = IntegerSchema::new("An integer.").schema();
static ARRAY: Schema = ArraySchema::new("Integers.", &INTEGER).schema();
static A: Schema = ObjectSchema::new(
    "Numeric variant.",
    &[("alpha", false, &INTEGER), ("items", true, &ARRAY)],
)
.schema();
static B: Schema = ObjectSchema::new("String variant.", &[("alpha", false, &STRING)]).schema();
static CHOICE: Schema = OneOfSchema::new(
    "Choice.",
    &("kind", false, &STRING),
    &[("a", &A), ("b", &B)],
)
.schema();
static ALL: Schema = AllOfSchema::new("Flattened choice.", &[&CHOICE]).schema();

fn config(schema: &'static Schema) -> SectionConfig {
    let mut config = SectionConfig::new(&STRING);
    config.register_plugin(SectionConfigPlugin::new(
        "entry".into(),
        None,
        schema.any_object().unwrap(),
    ));
    config
}

#[test]
fn types_can_follow_their_variant_properties() {
    // like in written configs, which have sorted properties
    let data = config(&ALL)
        .parse("test.cfg", "entry: first\n\talpha text\n\tkind b\n")
        .unwrap();
    assert_eq!(
        data.lookup_json("entry", "first").unwrap(),
        json!({"kind": "b", "alpha": "text"})
    );
}

#[test]
fn buffered_properties_keep_order_and_line_numbers() {
    let config = config(&ALL);
    let raw = concat!(
        "entry: first\n\titems 3\n\talpha 42\n\titems 1\n\tkind a\n",
        "\n",
        "entry: second\n\talpha text\n\tkind b\n",
    );
    let data = config.parse("test.cfg", raw).unwrap();
    assert_eq!(data.order, ["first", "second"]);
    assert_eq!(
        data.lookup_json("entry", "first").unwrap(),
        json!({"kind": "a", "alpha": 42, "items": [3, 1]})
    );

    for (raw, line) in [
        ("entry: first\n\talpha bad\n\tkind a\n", 2),
        (
            "entry: first\n\tkind a\n\talpha 42\n\nentry: second\n\talpha bad\n\tkind a\n",
            6,
        ),
    ] {
        let error = config.parse("test.cfg", raw).unwrap_err().to_string();
        assert!(
            error.contains(&format!("line {line} - property 'alpha'")),
            "{error}"
        );
    }
}

#[test]
fn unknown_types_are_rejected() {
    assert!(
        config(&ALL)
            .parse("test.cfg", "entry: first\n\tkind unknown\n")
            .is_err()
    );
}

#[test]
fn additional_properties_do_not_bypass_variant_schemas() {
    static OPEN: Schema = ObjectSchema::new("Extra properties.", &[])
        .additional_properties(true)
        .schema();
    static SCHEMA: Schema = AllOfSchema::new("Open choice.", &[&ALL, &OPEN]).schema();
    // `alpha` must not become an additional string property
    let data = config(&SCHEMA)
        .parse("test.cfg", "entry: first\n\talpha 42\n\tkind a\n")
        .unwrap();
    assert_eq!(
        data.lookup_json("entry", "first").unwrap(),
        json!({"kind": "a", "alpha": 42})
    );
}

#[test]
fn nested_types_can_follow_their_variant_properties() {
    static OUTER: Schema = OneOfSchema::new(
        "Outer choice.",
        &("outer", false, &STRING),
        &[("nested", &ALL)],
    )
    .schema();
    let data = config(&OUTER)
        .parse(
            "test.cfg",
            "entry: first\n\talpha 42\n\tkind a\n\touter nested\n",
        )
        .unwrap();
    assert_eq!(
        data.lookup_json("entry", "first").unwrap(),
        json!({"outer": "nested", "kind": "a", "alpha": 42})
    );
}

#[test]
fn header_types_select_the_variant() {
    let mut config = SectionConfig::new(&STRING);
    config.register_plugin(
        SectionConfigPlugin::new("a".into(), None, ALL.any_object().unwrap()).with_type_key("kind"),
    );
    let data = config.parse("test.cfg", "a: first\n\talpha 42\n").unwrap();
    assert_eq!(
        data.lookup_json("a", "first").unwrap(),
        json!({"kind": "a", "alpha": 42})
    );
}
