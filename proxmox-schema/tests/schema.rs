use anyhow::bail;
use serde_json::{Value, json};
use url::form_urlencoded;

use proxmox_schema::*;

fn parse_query_string<T: Into<ParameterSchema>>(
    query: &str,
    schema: T,
    test_required: bool,
) -> Result<Value, ParameterError> {
    let param_list: Vec<(String, String)> = form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();

    schema
        .into()
        .parse_parameter_strings(&param_list, test_required)
}

#[test]
fn test_schema1() {
    let schema = ObjectSchema::new("TEST", &[]).schema();

    println!("TEST Schema: {schema:?}");
}

#[test]
fn test_query_string() {
    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[("name", false, &StringSchema::new("Name.").schema())],
        );

        let res = parse_query_string("", &SCHEMA, true);
        assert!(res.is_err());
    }

    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[("name", true, &StringSchema::new("Name.").schema())],
        );

        let res = parse_query_string("", &SCHEMA, true);
        assert!(res.is_ok());
    }

    // TEST min_length and max_length
    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[(
                "name",
                true,
                &StringSchema::new("Name.")
                    .min_length(5)
                    .max_length(10)
                    .schema(),
            )],
        );

        let res = parse_query_string("name=abcd", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("name=abcde", &SCHEMA, true);
        assert!(res.is_ok());

        let res = parse_query_string("name=abcdefghijk", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("name=abcdefghij", &SCHEMA, true);
        assert!(res.is_ok());
    }

    // TEST regex pattern
    crate::const_regex! {
        TEST_REGEX = "test";
        TEST2_REGEX = "^test$";
    }

    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[(
                "name",
                false,
                &StringSchema::new("Name.")
                    .format(&ApiStringFormat::Pattern(&TEST_REGEX))
                    .schema(),
            )],
        );

        let res = parse_query_string("name=abcd", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("name=ateststring", &SCHEMA, true);
        assert!(res.is_ok());
    }

    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[(
                "name",
                false,
                &StringSchema::new("Name.")
                    .format(&ApiStringFormat::Pattern(&TEST2_REGEX))
                    .schema(),
            )],
        );

        let res = parse_query_string("name=ateststring", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("name=test", &SCHEMA, true);
        assert!(res.is_ok());
    }

    // TEST string enums
    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[(
                "name",
                false,
                &StringSchema::new("Name.")
                    .format(&ApiStringFormat::Enum(&[
                        EnumEntry::new("ev1", "desc ev1"),
                        EnumEntry::new("ev2", "desc ev2"),
                    ]))
                    .schema(),
            )],
        );

        let res = parse_query_string("name=noenum", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("name=ev1", &SCHEMA, true);
        assert!(res.is_ok());

        let res = parse_query_string("name=ev2", &SCHEMA, true);
        assert!(res.is_ok());

        let res = parse_query_string("name=ev3", &SCHEMA, true);
        assert!(res.is_err());
    }
}

#[test]
fn test_query_integer() {
    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[("count", false, &IntegerSchema::new("Count.").schema())],
        );

        let res = parse_query_string("", &SCHEMA, true);
        assert!(res.is_err());
    }

    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[(
                "count",
                true,
                &IntegerSchema::new("Count.")
                    .minimum(-3)
                    .maximum(50)
                    .schema(),
            )],
        );

        let res = parse_query_string("", &SCHEMA, true);
        assert!(res.is_ok());

        let res = parse_query_string("count=abc", &SCHEMA, false);
        assert!(res.is_err());

        let res = parse_query_string("count=30", &SCHEMA, false);
        assert!(res.is_ok());

        let res = parse_query_string("count=-1", &SCHEMA, false);
        assert!(res.is_ok());

        let res = parse_query_string("count=300", &SCHEMA, false);
        assert!(res.is_err());

        let res = parse_query_string("count=-30", &SCHEMA, false);
        assert!(res.is_err());

        let res = parse_query_string("count=50", &SCHEMA, false);
        assert!(res.is_ok());

        let res = parse_query_string("count=-3", &SCHEMA, false);
        assert!(res.is_ok());
    }
}

#[test]
fn test_query_boolean() {
    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[("force", false, &BooleanSchema::new("Force.").schema())],
        );

        let res = parse_query_string("", &SCHEMA, true);
        assert!(res.is_err());
    }

    {
        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[("force", true, &BooleanSchema::new("Force.").schema())],
        );

        let res = parse_query_string("", &SCHEMA, true);
        assert!(res.is_ok());

        let res = parse_query_string("a=b", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("force", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("force=yes", &SCHEMA, true);
        assert!(res.is_ok());
        let res = parse_query_string("force=1", &SCHEMA, true);
        assert!(res.is_ok());
        let res = parse_query_string("force=On", &SCHEMA, true);
        assert!(res.is_ok());
        let res = parse_query_string("force=TRUE", &SCHEMA, true);
        assert!(res.is_ok());
        let res = parse_query_string("force=TREU", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("force=NO", &SCHEMA, true);
        assert!(res.is_ok());
        let res = parse_query_string("force=0", &SCHEMA, true);
        assert!(res.is_ok());
        let res = parse_query_string("force=off", &SCHEMA, true);
        assert!(res.is_ok());
        let res = parse_query_string("force=False", &SCHEMA, true);
        assert!(res.is_ok());
    }
}

#[test]
fn test_verify_function() {
    const SCHEMA: ObjectSchema = ObjectSchema::new(
        "Parameters.",
        &[(
            "p1",
            false,
            &StringSchema::new("P1")
                .format(&ApiStringFormat::VerifyFn(|value| {
                    if value == "test" {
                        return Ok(());
                    };
                    bail!("format error");
                }))
                .schema(),
        )],
    );

    let res = parse_query_string("p1=tes", &SCHEMA, true);
    assert!(res.is_err());
    let res = parse_query_string("p1=test", &SCHEMA, true);
    assert!(res.is_ok());
}

#[test]
fn test_verify_complex_object() {
    const NIC_MODELS: ApiStringFormat = ApiStringFormat::Enum(&[
        EnumEntry::new("e1000", "Intel E1000"),
        EnumEntry::new("virtio", "Paravirtualized ethernet device"),
    ]);

    const PARAM_SCHEMA: Schema = ObjectSchema::new(
        "Properties.",
        &[
            (
                "enable",
                true,
                &BooleanSchema::new("Enable device.").schema(),
            ),
            (
                "model",
                false,
                &StringSchema::new("Ethernet device Model.")
                    .format(&NIC_MODELS)
                    .schema(),
            ),
        ],
    )
    .default_key("model")
    .schema();

    const SCHEMA: ObjectSchema = ObjectSchema::new(
        "Parameters.",
        &[(
            "net0",
            false,
            &StringSchema::new("First Network device.")
                .format(&ApiStringFormat::PropertyString(&PARAM_SCHEMA))
                .schema(),
        )],
    );

    let res = parse_query_string("", &SCHEMA, true);
    assert!(res.is_err());

    let res = parse_query_string("test=abc", &SCHEMA, true);
    assert!(res.is_err());

    let res = parse_query_string("net0=model=abc", &SCHEMA, true);
    assert!(res.is_err());

    let res = parse_query_string("net0=model=virtio", &SCHEMA, true);
    assert!(res.is_ok());

    let res = parse_query_string("net0=model=virtio,enable=1", &SCHEMA, true);
    assert!(res.is_ok());

    let res = parse_query_string("net0=virtio,enable=no", &SCHEMA, true);
    assert!(res.is_ok());
}

#[test]
fn test_verify_complex_array() {
    {
        const PARAM_SCHEMA: Schema =
            ArraySchema::new("Integer List.", &IntegerSchema::new("Something").schema()).schema();

        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[(
                "list",
                false,
                &StringSchema::new("A list on integers, comma separated.")
                    .format(&ApiStringFormat::PropertyString(&PARAM_SCHEMA))
                    .schema(),
            )],
        );

        let res = parse_query_string("", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("list=", &SCHEMA, true);
        assert!(res.is_ok());

        let res = parse_query_string("list=abc", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("list=1", &SCHEMA, true);
        assert!(res.is_ok());

        let res = parse_query_string("list=2,3,4,5", &SCHEMA, true);
        assert!(res.is_ok());
    }

    {
        const PARAM_SCHEMA: Schema =
            ArraySchema::new("Integer List.", &IntegerSchema::new("Something").schema())
                .min_length(1)
                .max_length(3)
                .schema();

        const SCHEMA: ObjectSchema = ObjectSchema::new(
            "Parameters.",
            &[(
                "list",
                false,
                &StringSchema::new("A list on integers, comma separated.")
                    .format(&ApiStringFormat::PropertyString(&PARAM_SCHEMA))
                    .schema(),
            )],
        );

        let res = parse_query_string("list=", &SCHEMA, true);
        assert!(res.is_err());

        let res = parse_query_string("list=1,2,3", &SCHEMA, true);
        assert!(res.is_ok());

        let res = parse_query_string("list=2,3,4,5", &SCHEMA, true);
        assert!(res.is_err());
    }
}

#[test]
fn test_property_aliases() {
    const SCHEMA: ObjectSchema = ObjectSchema::new(
        "Parameters.",
        &[
            ("count", true, &IntegerSchema::new("Count.").schema()),
            ("mode", false, &StringSchema::new("Mode.").schema()),
            (
                "tags",
                true,
                &ArraySchema::new("Tags.", &StringSchema::new("Tag.").schema()).schema(),
            ),
        ],
    )
    .property_aliases(&[
        ("deprecated-mode", "mode"),
        ("legacy-mode", "mode"),
        ("nr", "count"),
        ("tag", "tags"),
    ]);

    let res = parse_query_string("mode=fast", &SCHEMA, true).expect("canonical");
    assert_eq!(res["mode"], "fast");

    let res = parse_query_string("deprecated-mode=fast", &SCHEMA, true).expect("alias");
    assert_eq!(res["mode"], "fast");
    assert!(
        res.get("deprecated-mode").is_none(),
        "alias key must be rewritten"
    );

    let res = parse_query_string("deprecated-mode=fast&nr=3", &SCHEMA, true).expect("aliases");
    assert_eq!(res["mode"], "fast");
    assert_eq!(res["count"], 3);

    let err =
        parse_query_string("mode=fast&deprecated-mode=slow", &SCHEMA, true).expect_err("conflict");
    assert!(
        format!("{err}").contains("cannot set both"),
        "unexpected error: {err}"
    );

    // Reverse order: alias first, canonical second. Must report the same conflict, not the
    // generic "duplicate parameter" message.
    let err = parse_query_string("deprecated-mode=fast&mode=slow", &SCHEMA, true)
        .expect_err("reverse-order conflict");
    let err_text = format!("{err}");
    assert!(
        err_text.contains("cannot set both"),
        "reverse-order should report `cannot set both`, got: {err_text}"
    );
    assert!(
        !err_text.contains("duplicate parameter"),
        "reverse-order must not regress to `duplicate parameter`, got: {err_text}"
    );

    // Two distinct aliases of the same canonical: also rejected.
    let err = parse_query_string("deprecated-mode=fast&legacy-mode=slow", &SCHEMA, true)
        .expect_err("two-alias conflict");
    assert!(
        format!("{err}").contains("cannot set both"),
        "two aliases of same canonical must conflict: {err}"
    );

    // Array property via an alias works, and mixing alias with canonical for an array is also
    // rejected (the bug that the array branch previously merged silently).
    let res =
        parse_query_string("mode=fast&tag=foo&tag=bar", &SCHEMA, true).expect("array via alias");
    assert_eq!(res["tags"], json!(["foo", "bar"]));
    let err =
        parse_query_string("mode=fast&tag=foo&tags=bar", &SCHEMA, true).expect_err("array mix");
    assert!(
        format!("{err}").contains("cannot set both"),
        "array branch must reject set-both: {err}"
    );

    SCHEMA
        .verify_json(&json!({"mode": "fast"}))
        .expect("canonical verify");
    SCHEMA
        .verify_json(&json!({"deprecated-mode": "fast"}))
        .expect("alias satisfies required");
    SCHEMA
        .verify_json(&json!({"mode": "fast", "deprecated-mode": "slow"}))
        .expect_err("conflict via verify_json");

    // canonicalize_aliases rewrites the alias key in place, so downstream consumers that look up
    // only the canonical name see the value. This is what the REST server now does before
    // dispatching the macro-generated handler.
    let mut body = json!({"deprecated-mode": "fast"});
    SCHEMA
        .canonicalize_aliases(&mut body)
        .expect("canonicalize rewrites alias");
    assert_eq!(body["mode"], "fast");
    assert!(body.get("deprecated-mode").is_none());

    // canonicalize_aliases catches both-set and leaves the value unchanged.
    let mut body = json!({"deprecated-mode": "fast", "mode": "slow"});
    let original = body.clone();
    SCHEMA
        .canonicalize_aliases(&mut body)
        .expect_err("canonicalize rejects both-set");
    assert_eq!(body, original, "value must not be mutated on conflict");

    // Same-canonical via two different aliases is also caught.
    let mut body = json!({"deprecated-mode": "fast", "legacy-mode": "slow"});
    SCHEMA
        .canonicalize_aliases(&mut body)
        .expect_err("canonicalize rejects two-alias");
}

#[test]
fn test_property_aliases_in_all_of() {
    const LEFT: Schema = ObjectSchema::new(
        "Left.",
        &[("mode", false, &StringSchema::new("Mode.").schema())],
    )
    .property_aliases(&[("legacy-mode", "mode")])
    .schema();
    const RIGHT: Schema = ObjectSchema::new(
        "Right.",
        &[("count", true, &IntegerSchema::new("Count.").schema())],
    )
    .schema();
    const COMBINED: AllOfSchema = AllOfSchema::new("Combined.", &[&LEFT, &RIGHT]);

    let res = parse_query_string("legacy-mode=fast", &COMBINED, true).expect("alias in allof");
    assert_eq!(res["mode"], "fast");

    let mut body = json!({"legacy-mode": "fast", "count": 1});
    COMBINED
        .canonicalize_aliases(&mut body)
        .expect("canonicalize via AllOf");
    assert_eq!(body["mode"], "fast");
    assert!(body.get("legacy-mode").is_none());
}

mod one_of_type_property_clash {
    use proxmox_schema::{ObjectSchema, OneOfSchema, Schema, StringSchema};

    const SCHEMA_A: Schema = ObjectSchema::new(
        "A.",
        &[("ty", false, &StringSchema::new("Type clash.").schema())],
    )
    .schema();
    const SCHEMA_B: Schema = ObjectSchema::new(
        "B.",
        &[("prop", false, &StringSchema::new("A property.").schema())],
    )
    .schema();
    const TYPE_SCHEMA: Schema = StringSchema::new("The type property.").schema();

    #[test]
    #[should_panic(
        expected = "a oneOf variant contains a property with the same name as the type property"
    )]
    fn type_property_in_variant() {
        let _ = OneOfSchema::new(
            "Test.",
            &("ty", false, &TYPE_SCHEMA),
            &[("A", &SCHEMA_A), ("B", &SCHEMA_B)],
        );
    }
}

#[cfg(feature = "test-harness")]
mod one_of_test_schema {
    use proxmox_api_macro::json_schema;
    use proxmox_schema::*;

    pub const PROP_NAME: Schema = StringSchema::new("A name.").schema();
    pub const PROP_VALUE: Schema = IntegerSchema::new("An integer.").schema();

    pub const TYPE_PROPERTY: Schema = StringSchema::new("A or B")
        .format(&ApiStringFormat::Enum(&[
            EnumEntry::new("A", "Type A"),
            EnumEntry::new("B", "Type B"),
        ]))
        .schema();

    pub const PROP_A: Schema = StringSchema::new("Property for A.").schema();
    pub const PROP_CLASHING_A: Schema =
        BooleanSchema::new("A common clashing schema (A).").schema();
    pub const INSTANCE_A: Schema = json_schema! {
        description: "An instance of A",
        properties: {
            "prop-a": { schema: PROP_A },
            "clashing": {
                optional: true,
                schema: PROP_CLASHING_A,
            },
        },
    };

    pub const PROP_B: Schema = StringSchema::new("Property for B").schema();
    pub const PROP_CLASHING_B: Schema =
        IntegerSchema::new("A common clashing schema (B).").schema();
    pub const INSTANCE_B: Schema = json_schema! {
        description: "An instance of B",
        properties: {
            "prop-b": { schema: PROP_B },
            "clashing": { schema: PROP_CLASHING_B },
        },
    };

    pub const INNER_TYPE_PROPERTY: Schema = StringSchema::new("B1 or B2")
        .format(&ApiStringFormat::Enum(&[
            EnumEntry::new("B1", "Type B1"),
            EnumEntry::new("B2", "Type B2"),
        ]))
        .schema();

    pub const PROP_B1: Schema = StringSchema::new("Property B1").schema();
    pub const INSTANCE_B1: Schema = json_schema! {
        description: "An instance of B1",
        properties: {
            "prop-b1": { schema: PROP_B1 },
        },
    };

    pub const PROP_B2: Schema = StringSchema::new("Property B2").schema();
    pub const INSTANCE_B2: Schema = json_schema! {
        description: "An instance of B2",
        properties: {
            "prop-b2": { schema: PROP_B2 },
        },
    };

    pub const ONE_OF_SCHEMA: Schema = json_schema! {
        description: "An A or B.",
        "type-property": "type",
        "type-property-schema": { schema: TYPE_PROPERTY },
        oneOf: [
            {
                "instance-type": "A",
                schema: INSTANCE_A,
            },
            {
                "instance-type": "B",
                description: "Instance type B",
                allOf: [
                    { schema: INSTANCE_B },
                    {
                        description: "A B1 or B2",
                        "type-property": "inner-type",
                        "type-property-schema": { schema: INNER_TYPE_PROPERTY },
                        oneOf: [
                            {
                                "instance-type": "B1",
                                schema: INSTANCE_B1,
                            },
                            {
                                "instance-type": "B2",
                                schema: INSTANCE_B2,
                            },
                        ],
                    },
                ],
            },
        ],
    };

    pub const SCHEMA: Schema = json_schema! {
        description: "Collection of schemas",
        allOf: [
            {
                description: "Simple Schema",
                properties: {
                    name: { schema: PROP_NAME },
                    value: {
                        schema: PROP_VALUE,
                        optional: true,
                    },
                },
            },
            { schema: ONE_OF_SCHEMA },
        ],
    };
}

#[cfg(feature = "test-harness")]
#[test]
fn test_lookup_schema_with_data() {
    use proxmox_api_macro::json_schema;

    use one_of_test_schema::*;

    let data_a = serde_json::json!({
        "type": "A",
        "prop-a": "hello",
    });

    let schema = SCHEMA.any_object().expect("schema should be an object");

    assert_eq!(
        schema.lookup_schema("name", &NoTypeData),
        Some((
            false,
            &json_schema! {
                "type": String,
                description: "A name.",
            }
        )),
        "Regular property lookup should work, optional=false",
    );
    assert_eq!(
        schema.lookup_schema("value", &NoTypeData),
        Some((
            true,
            &json_schema! {
                "type": Integer,
                description: "An integer.",
            }
        )),
        "Regular property lookup should work, optional=true",
    );
    assert_eq!(
        schema.lookup_schema("prop-a", &NoTypeData),
        None,
        "Looking up a property inside a oneOf should yield nothing",
    );
    assert_eq!(
        schema.lookup_schema("prop-b", &NoTypeData),
        None,
        "Looking up a property inside a oneOf should yield nothing",
    );
    assert_eq!(
        schema.lookup_schema("clashing", &NoTypeData),
        None,
        "Looking up a property inside a oneOf should yield nothing",
    );

    assert_eq!(
        schema.lookup_schema("prop-a", &data_a),
        Some((false, &PROP_A)),
        "Looking up a property which is valid for the data should work",
    );
    assert_eq!(
        schema.lookup_schema("prop-b", &data_a),
        None,
        "Looking up a property which is invalid for the data should yield nothing",
    );

    let data_b = serde_json::json!({
        "type": "B",
        "prop-b": "hello",
    });

    assert_eq!(
        schema.lookup_schema("prop-a", &data_b),
        None,
        "Looking up a property which is invalid for the data should yield nothing",
    );
    assert_eq!(
        schema.lookup_schema("prop-b", &data_b),
        Some((false, &PROP_B)),
        "Looking up a property which is valid for the data should work",
    );

    assert_eq!(
        schema.lookup_schema("clashing", &data_a),
        Some((true, &PROP_CLASHING_A)),
        "Looking up clashing keys should yield the correct entry (A).",
    );
    assert_eq!(
        schema.lookup_schema("clashing", &data_b),
        Some((false, &PROP_CLASHING_B)),
        "Looking up clashing keys should yield the correct entry (B).",
    );

    let data_b1 = serde_json::json!({
        "type": "B",
        "prop-b": "hello",
        "inner-type": "B1",
    });
    assert_eq!(
        schema.lookup_schema("prop-b1", &data_b1),
        Some((false, &PROP_B1)),
        "Nested oneOf lookups should work",
    );
    assert_eq!(
        schema.lookup_schema("prop-b2", &data_b1),
        None,
        "Nested oneOf lookups should work",
    );

    let data_b2 = serde_json::json!({
        "type": "B",
        "prop-b": "hello",
        "inner-type": "B2",
    });
    assert_eq!(
        schema.lookup_schema("prop-b1", &data_b2),
        None,
        "Nested oneOf lookups should work",
    );
    assert_eq!(
        schema.lookup_schema("prop-b2", &data_b2),
        Some((false, &PROP_B2)),
        "Nested oneOf lookups should work",
    );
}

#[cfg(feature = "test-harness")]
#[test]
fn test_all_properties_iterator() {
    use one_of_test_schema::*;

    type ExpectedTuple<'a> = (&'a str, bool, &'static Schema, &'a [(&'a str, &'a str)]);

    #[track_caller]
    fn compare_iter(
        iter: proxmox_schema::AllProperties,
        expected: &[ExpectedTuple],
        msg: &'static str,
    ) {
        let properties = iter.collect::<Vec<_>>();
        assert_eq!(
            properties.len(),
            expected.len(),
            "{msg} - number of elements"
        );
        for (a, b) in properties.into_iter().zip(expected) {
            assert_eq!(
                (a.name(), a.optional(), a.schema(), a.conditions()),
                (b.0, b.1, b.2, b.3),
                "{msg} - matching entries",
            );
        }
    }

    let schema = SCHEMA.any_object().expect("schema should be an object");

    #[rustfmt::skip]
    compare_iter(
        schema.all_properties(),
        &[
            ("name",       false, &PROP_NAME,           &[]),
            ("value",      true,  &PROP_VALUE,          &[]),
            ("type",       false, &TYPE_PROPERTY,       &[]),
            ("clashing",   true,  &PROP_CLASHING_A,     &[("type", "A")]),
            ("prop-a",     false, &PROP_A,              &[("type", "A")]),
            ("clashing",   false, &PROP_CLASHING_B,     &[("type", "B")]),
            ("prop-b",     false, &PROP_B,              &[("type", "B")]),
            ("inner-type", false, &INNER_TYPE_PROPERTY, &[("type", "B")]),
            ("prop-b1",    false, &PROP_B1,             &[("type", "B"), ("inner-type", "B1")]),
            ("prop-b2",    false, &PROP_B2,             &[("type", "B"), ("inner-type", "B2")]),
        ],
        "generic iteration should yield all properties, including duplicates",
    );

    #[rustfmt::skip]
    compare_iter(
        schema.all_properties().with_types_from(&json!({ "type": "A" })),
        &[
            ("name",     false, &PROP_NAME,       &[]),
            ("value",    true,  &PROP_VALUE,      &[]),
            ("type",     false, &TYPE_PROPERTY,   &[]),
            ("clashing", true,  &PROP_CLASHING_A, &[("type", "A")]),
            ("prop-a",   false, &PROP_A,          &[("type", "A")]),
        ],
        "type-specific iteration should yield the correct types (A)",
    );

    #[rustfmt::skip]
    compare_iter(
        schema.all_properties().with_types_from(&json!({ "type": "B" })),
        &[
            ("name",       false, &PROP_NAME,           &[]),
            ("value",      true,  &PROP_VALUE,          &[]),
            ("type",       false, &TYPE_PROPERTY,       &[]),
            ("clashing",   false, &PROP_CLASHING_B,     &[("type", "B")]),
            ("prop-b",     false, &PROP_B,              &[("type", "B")]),
            ("inner-type", false, &INNER_TYPE_PROPERTY, &[("type", "B")]),
        ],
        "type-specific iteration should yield the correct types (B)",
    );

    #[rustfmt::skip]
    compare_iter(
        schema.all_properties().with_types_from(&json!({ "type": "B", "inner-type": "B1" })),
        &[
            ("name",       false, &PROP_NAME,           &[]),
            ("value",      true,  &PROP_VALUE,          &[]),
            ("type",       false, &TYPE_PROPERTY,       &[]),
            ("clashing",   false, &PROP_CLASHING_B,     &[("type", "B")]),
            ("prop-b",     false, &PROP_B,              &[("type", "B")]),
            ("inner-type", false, &INNER_TYPE_PROPERTY, &[("type", "B")]),
            ("prop-b1",    false, &PROP_B1,             &[("type", "B"), ("inner-type", "B1")]),
        ],
        "type-specific iteration should yield the correct types (B, B1)",
    );

    #[rustfmt::skip]
    compare_iter(
        schema.all_properties().with_types_from(&json!({ "type": "B", "inner-type": "B2" })),
        &[
            ("name",       false, &PROP_NAME,           &[]),
            ("value",      true,  &PROP_VALUE,          &[]),
            ("type",       false, &TYPE_PROPERTY,       &[]),
            ("clashing",   false, &PROP_CLASHING_B,     &[("type", "B")]),
            ("prop-b",     false, &PROP_B,              &[("type", "B")]),
            ("inner-type", false, &INNER_TYPE_PROPERTY, &[("type", "B")]),
            ("prop-b2",    false, &PROP_B2,             &[("type", "B"), ("inner-type", "B2")]),
        ],
        "type-specific iteration should yield the correct types (B, B2)",
    );

    #[rustfmt::skip]
    compare_iter(
        schema.all_properties().with_types_from(&json!({ "type": "INVALID" })),
        &[
            ("name",     false, &PROP_NAME,       &[]),
            ("value",    true,  &PROP_VALUE,      &[]),
            ("type",     false, &TYPE_PROPERTY,   &[]),
        ],
        "invalid-type iteration should not yield the oneOf contents",
    );

    #[rustfmt::skip]
    compare_iter(
        schema.all_properties().with_types_from(&json!({})),
        &[
            ("name",     false, &PROP_NAME,       &[]),
            ("value",    true,  &PROP_VALUE,      &[]),
            ("type",     false, &TYPE_PROPERTY,   &[]),
        ],
        "missing-type iteration should not yield the oneOf contents",
    );

    let schema = ONE_OF_SCHEMA
        .any_object()
        .expect("schema should be an object");
    #[rustfmt::skip]
    compare_iter(
        schema.all_properties(),
        &[
            ("type",       false, &TYPE_PROPERTY,       &[]),
            ("clashing",   true,  &PROP_CLASHING_A,     &[("type", "A")]),
            ("prop-a",     false, &PROP_A,              &[("type", "A")]),
            ("clashing",   false, &PROP_CLASHING_B,     &[("type", "B")]),
            ("prop-b",     false, &PROP_B,              &[("type", "B")]),
            ("inner-type", false, &INNER_TYPE_PROPERTY, &[("type", "B")]),
            ("prop-b1",    false, &PROP_B1,             &[("type", "B"), ("inner-type", "B1")]),
            ("prop-b2",    false, &PROP_B2,             &[("type", "B"), ("inner-type", "B2")]),
        ],
        "generic iteration should yield all properties, including duplicates",
    );

    #[rustfmt::skip]
    compare_iter(
        schema.all_properties().with_types_from(&json!({ "type": "A" })),
        &[
            ("type",     false, &TYPE_PROPERTY,   &[]),
            ("clashing", true,  &PROP_CLASHING_A, &[("type", "A")]),
            ("prop-a",   false, &PROP_A,          &[("type", "A")]),
        ],
        "type-specific iteration should yield the correct types (A)",
    );

    #[rustfmt::skip]
    compare_iter(
        schema.all_properties().with_types_from(&json!({ "type": "B" })),
        &[
            ("type",       false, &TYPE_PROPERTY,       &[]),
            ("clashing",   false, &PROP_CLASHING_B,     &[("type", "B")]),
            ("prop-b",     false, &PROP_B,              &[("type", "B")]),
            ("inner-type", false, &INNER_TYPE_PROPERTY, &[("type", "B")]),
        ],
        "type-specific iteration should yield the correct types (B)",
    );
}
