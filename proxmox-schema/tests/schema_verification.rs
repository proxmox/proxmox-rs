use anyhow::{Error, bail};
use serde::Deserialize;
use serde_json::{Value, json};

use proxmox_schema::*;

static STRING_SCHEMA: Schema = StringSchema::new("A test string").schema();

static SIMPLE_OBJECT_SCHEMA: Schema = ObjectSchema::new(
    "simple object schema",
    &[
        ("prop1", false, &STRING_SCHEMA),
        ("prop2", true, &STRING_SCHEMA),
        ("prop3", false, &STRING_SCHEMA),
    ],
)
.schema();

static SIMPLE_PROPERTY_STRING_SCHEMA: Schema = StringSchema::new("simple property string")
    .format(&ApiStringFormat::PropertyString(&SIMPLE_OBJECT_SCHEMA))
    .schema();

static SIMPLE_ARRAY_SCHEMA: Schema = ArraySchema::new("String list.", &STRING_SCHEMA).schema();

static NESTED_OBJECT_SCHEMA: Schema = ObjectSchema::new(
    "nested object schema",
    &[
        ("arr1", false, &SIMPLE_ARRAY_SCHEMA),
        ("obj1", false, &SIMPLE_OBJECT_SCHEMA),
        ("prop1", false, &STRING_SCHEMA),
    ],
)
.schema();

static NESTED_PROPERTY_SCHEMA: Schema = ObjectSchema::new(
    "object with property strings",
    &[("ps1", false, &SIMPLE_PROPERTY_STRING_SCHEMA)],
)
.schema();

static ANOTHER_OBJECT_SCHEMA: Schema = ObjectSchema::new(
    "another simple object schema",
    &[
        ("another1", false, &STRING_SCHEMA),
        ("another2", true, &STRING_SCHEMA),
    ],
)
.schema();

static OBJECT_WITH_ADDITIONAL: Schema = ObjectSchema::new(
    "object allowing additional properties",
    &[
        ("regular1", false, &STRING_SCHEMA),
        ("regular2", true, &STRING_SCHEMA),
    ],
)
.additional_properties(true)
.schema();

static ALL_OF_SCHEMA_NO_ADDITIONAL: Schema = AllOfSchema::new(
    "flattening 2 objects together",
    &[&SIMPLE_OBJECT_SCHEMA, &ANOTHER_OBJECT_SCHEMA],
)
.schema();

static ALL_OF_SCHEMA_ADDITIONAL: Schema = AllOfSchema::new(
    "flattening 2 objects together where 1 allows additional properties",
    &[&SIMPLE_OBJECT_SCHEMA, &OBJECT_WITH_ADDITIONAL],
)
.schema();

static PROP_COMMON_1: Schema = StringSchema::new("Common 1").schema();
static PROP_COMMON_2: Schema = IntegerSchema::new("Common 2").schema();
static PROP_ONE: Schema = StringSchema::new("Property 1").schema();
static PROP_TWO: Schema = IntegerSchema::new("Property 2").schema();
static PROP_CLASHING_A: Schema = StringSchema::new("Clashing Property 1").schema();
static PROP_CLASHING_B: Schema = IntegerSchema::new("Clashing Property 2").schema();

static PROP_ENUM_TYPE: Schema = StringSchema::new("Type: A or B")
    .format(&ApiStringFormat::Enum(&[
        EnumEntry::new("A", "Type A"),
        EnumEntry::new("B", "Type B"),
    ]))
    .schema();

static ENUM_COMMON: Schema = ObjectSchema::new(
    "Common Object Data",
    &[
        ("common1", false, &PROP_COMMON_1),
        ("common2", true, &PROP_COMMON_2),
    ],
)
.additional_properties(false)
.schema();

static ENUM_A: Schema = ObjectSchema::new(
    "Type A Object",
    &[
        ("clashing", false, &PROP_CLASHING_A),
        ("prop-one", false, &PROP_ONE),
    ],
)
.additional_properties(false)
.schema();

static ENUM_B: Schema = ObjectSchema::new(
    "Type B Object",
    &[
        ("clashing", false, &PROP_CLASHING_B),
        ("prop-two", false, &PROP_TWO),
    ],
)
.additional_properties(false)
.schema();

static ENUM_ONE_OF: Schema = OneOfSchema::new(
    "Object A or B.",
    &("type", false, &PROP_ENUM_TYPE),
    &[("A", &ENUM_A), ("B", &ENUM_B)],
)
.schema();

static ENUM_SCHEMA: Schema = AllOfSchema::new(
    "An enum with common and clashing values.",
    &[&ENUM_COMMON, &ENUM_ONE_OF],
)
.schema();

#[track_caller]
fn compare_error(expected: &[(&str, &str)], err: Error) -> Result<(), Error> {
    let err = match err.downcast_ref::<ParameterError>() {
        Some(err) => err,
        None => panic!("unable to downcast error: {err}"),
    };

    let result = (move || {
        let errors = err.errors();

        if errors.len() != expected.len() {
            bail!(
                "error list has different length: {} != {}",
                expected.len(),
                errors.len()
            );
        }

        for i in 0..expected.len() {
            if expected[i].0 != errors[i].0 {
                bail!(
                    "error {} path differs: '{}' != '{}'",
                    i,
                    expected[i].0,
                    errors[i].0
                );
            }
            if expected[i].1 != errors[i].1.to_string() {
                bail!(
                    "error {} message differs: '{}' != '{}'",
                    i,
                    expected[i].1,
                    errors[i].1
                );
            }
        }

        Ok(())
    })();

    if result.is_err() {
        println!("GOT: {err:?}");
    }

    result
}

fn test_verify(
    schema: &Schema,
    data: &Value,
    expected_errors: &[(&str, &str)],
) -> Result<(), Error> {
    match schema.verify_json(data) {
        Ok(_) => bail!("expected errors, but got Ok()"),
        Err(err) => compare_error(expected_errors, err)?,
    }
    Ok(())
}

#[test]
fn verify_simple_object() -> Result<(), Error> {
    let simple_value = json!({"prop1": 1, "prop4": "abc"});

    test_verify(
        &SIMPLE_OBJECT_SCHEMA,
        &simple_value,
        &[
            ("prop1", "Expected string value."),
            ("prop4", "schema does not allow additional properties"),
            ("prop3", "property is missing and it is not optional"),
        ],
    )?;

    Ok(())
}

#[test]
fn verify_nested_object1() -> Result<(), Error> {
    let nested_value = json!({"prop1": 1, "prop4": "abc"});

    test_verify(
        &NESTED_OBJECT_SCHEMA,
        &nested_value,
        &[
            ("prop1", "Expected string value."),
            ("prop4", "schema does not allow additional properties"),
            ("arr1", "property is missing and it is not optional"),
            ("obj1", "property is missing and it is not optional"),
        ],
    )?;

    Ok(())
}

#[test]
fn verify_nested_object2() -> Result<(), Error> {
    let nested_value = json!({"prop1": 1, "prop4": "abc", "obj1": {}, "arr1": ["abc", 0]});

    test_verify(
        &NESTED_OBJECT_SCHEMA,
        &nested_value,
        &[
            ("arr1/[1]", "Expected string value."),
            ("obj1/prop1", "property is missing and it is not optional"),
            ("obj1/prop3", "property is missing and it is not optional"),
            ("prop1", "Expected string value."),
            ("prop4", "schema does not allow additional properties"),
        ],
    )?;

    Ok(())
}

#[test]
fn verify_nested_property1() -> Result<(), Error> {
    let value = json!({"ps1": "abc"});

    test_verify(
        &NESTED_PROPERTY_SCHEMA,
        &value,
        &[
            (
                "ps1",
                "value without key, but schema does not define a default key",
            ),
            ("ps1/prop1", "property is missing and it is not optional"),
            ("ps1/prop3", "property is missing and it is not optional"),
        ],
    )?;

    Ok(())
}

#[test]
fn verify_nested_property2() -> Result<(), Error> {
    let value = json!({"ps1": "abc=1"});

    test_verify(
        &NESTED_PROPERTY_SCHEMA,
        &value,
        &[
            ("ps1/abc", "schema does not allow additional properties"),
            ("ps1/prop1", "property is missing and it is not optional"),
            ("ps1/prop3", "property is missing and it is not optional"),
        ],
    )?;

    Ok(())
}

#[test]
fn verify_nested_property3() -> Result<(), Error> {
    let value = json!({"ps1": ""});

    test_verify(
        &NESTED_PROPERTY_SCHEMA,
        &value,
        &[
            ("ps1/prop1", "property is missing and it is not optional"),
            ("ps1/prop3", "property is missing and it is not optional"),
        ],
    )?;

    Ok(())
}

#[test]
fn verify_all_of_schema() -> Result<(), Error> {
    let value = json!({
        "prop1": "hello",
        "prop3": "hello",
        "another1": "another hello",
    });
    ALL_OF_SCHEMA_NO_ADDITIONAL
        .verify_json(&value)
        .expect("all of schema failed to verify valid object");

    let value = json!({
        "prop1": "hello",
        "prop3": "hello",
    });
    test_verify(
        &ALL_OF_SCHEMA_NO_ADDITIONAL,
        &value,
        &[("another1", "property is missing and it is not optional")],
    )?;

    let value = json!({
        "prop1": "hello",
        "prop3": "hello",
        "another1": "another hello",
        "additional": "additional value",
    });
    test_verify(
        &ALL_OF_SCHEMA_NO_ADDITIONAL,
        &value,
        &[("additional", "schema does not allow additional properties")],
    )?;

    Ok(())
}

#[test]
fn verify_all_of_schema_with_additional() -> Result<(), Error> {
    let value = json!({
        "prop1": "hello",
        "prop3": "hello",
        "regular1": "another hello",
        "more": "additional property",
    });
    ALL_OF_SCHEMA_ADDITIONAL
        .verify_json(&value)
        .expect("all of schema failed to verify valid object");

    let value = json!({
        "prop1": "hello",
        "prop3": "hello",
        "more": "additional property",
    });
    test_verify(
        &ALL_OF_SCHEMA_ADDITIONAL,
        &value,
        &[("regular1", "property is missing and it is not optional")],
    )?;

    Ok(())
}

#[test]
fn verify_one_of_schema_property_string() -> Result<(), Error> {
    de::verify(
        &ENUM_SCHEMA,
        "common1=hey,type=A,prop-one=Hello,clashing=Okay",
    )
    .expect("failed to verify oneOf type A");

    de::verify(
        &ENUM_SCHEMA,
        "common1=hey,prop-one=Hello,clashing=33,type=A",
    )
    .expect("failed to verify oneOf type A (numeric string)");

    compare_error(
        &[
            ("prop-one", "schema does not allow additional properties"),
            ("clashing", "not an integer: \"Okay\""),
            ("prop-two", "property is missing and it is not optional"),
        ],
        de::verify(
            &ENUM_SCHEMA,
            "common1=hey,type=B,prop-one=Hello,clashing=Okay",
        )
        .expect_err("wrong clashing key should fail"),
    )?;

    compare_error(
        &[("prop-two", "not an integer: \"hello\"")],
        de::verify(
            &ENUM_SCHEMA,
            "common1=hey,type=B,prop-two=hello,clashing=33",
        )
        .expect_err("wrong property type in a oneOf property should be caught"),
    )?;

    compare_error(
        &[("prop-two", "duplicate key")],
        de::verify(
            &ENUM_SCHEMA,
            "common1=hey,prop-two=33,clashing=33,prop-two=44,type=B",
        )
        .expect_err("duplicate keys should be caught"),
    )?;

    de::verify(&ENUM_SCHEMA, "common1=hey,type=B,prop-two=1,clashing=33")
        .expect("failed to verify valid oneOf type B");

    // serde type order test - these types follow the above-declared schema

    #[derive(Deserialize, Debug, PartialEq, Eq)]
    struct Full {
        common1: String,
        common2: Option<u64>,
        #[serde(flatten)]
        the_enum: AnEnum,
    }

    #[derive(Deserialize, Debug, PartialEq, Eq)]
    #[serde(tag = "type")]
    enum AnEnum {
        A(TypeA),
        B(TypeB),
    }

    #[derive(Deserialize, Debug, PartialEq, Eq)]
    #[serde(rename_all = "kebab-case")]
    struct TypeA {
        clashing: String,
        prop_one: String,
    }

    #[derive(Deserialize, Debug, PartialEq, Eq)]
    #[serde(rename_all = "kebab-case")]
    struct TypeB {
        clashing: u64,
        prop_two: u64,
    }

    assert_eq!(
        Full::deserialize(de::SchemaDeserializer::new(
            "common1=hey,prop-one=Hello,clashing=33,type=A",
            &ENUM_SCHEMA
        ))
        .expect("failed to deserialize into struct"),
        Full {
            common1: "hey".to_string(),
            common2: None,
            the_enum: AnEnum::A(TypeA {
                clashing: "33".to_string(),
                prop_one: "Hello".to_string(),
            })
        },
    );
    assert_eq!(
        Full::deserialize(de::SchemaDeserializer::new(
            "common1=hey,common2=11,prop-two=12,clashing=33,type=B",
            &ENUM_SCHEMA
        ))
        .expect("failed to deserialize into struct"),
        Full {
            common1: "hey".to_string(),
            common2: Some(11),
            the_enum: AnEnum::B(TypeB {
                clashing: 33,
                prop_two: 12,
            })
        },
    );

    Ok(())
}

mod property_string_aliases {
    use super::*;

    static NAME: Schema = ObjectSchema::new("Required name.", &[("name", false, &STRING_SCHEMA)])
        .property_aliases(&[("legacy-name", "name"), ("old-name", "name")])
        .schema();

    #[test]
    fn alias_satisfies_required_property() {
        de::verify(&NAME, "old-name=value").expect("alias must satisfy the required property");
    }

    #[test]
    fn aliases_conflict_with_other_spellings() -> Result<(), Error> {
        for (input, duplicate) in [
            ("name=new,old-name=old", "old-name"),
            ("old-name=old,name=new", "name"),
            ("legacy-name=old,old-name=older", "old-name"),
        ] {
            compare_error(
                &[(duplicate, "duplicate key")],
                de::verify(&NAME, input).expect_err("conflicting spellings must be rejected"),
            )?;
        }
        Ok(())
    }

    #[test]
    fn aliases_follow_the_active_variant() -> Result<(), Error> {
        static SCHEMA: Schema = OneOfSchema::new(
            "Variant-specific aliases.",
            &("type", false, &PROP_ENUM_TYPE),
            &[
                (
                    "A",
                    &ObjectSchema::new("A.", &[("alpha", false, &STRING_SCHEMA)])
                        .property_aliases(&[("old-name", "alpha")])
                        .schema(),
                ),
                (
                    "B",
                    &ObjectSchema::new("B.", &[("beta", false, &STRING_SCHEMA)])
                        .property_aliases(&[("old-name", "beta")])
                        .schema(),
                ),
            ],
        )
        .schema();

        // `old-name` means `alpha` in the first variant, and the type comes last
        de::verify(&SCHEMA, "old-name=value,type=B").expect("alias must satisfy `beta`");
        compare_error(
            &[("old-name", "duplicate key")],
            de::verify(&SCHEMA, "beta=new,old-name=old,type=B")
                .expect_err("alias must conflict with `beta`"),
        )
    }
}

#[test]
fn verify_array_strings_and_indices() -> Result<(), Error> {
    static LIMITED_ARRAY: Schema = ArraySchema::new(
        "Int List.",
        &IntegerSchema::new("Number").maximum(200).schema(),
    )
    .max_length(3)
    .schema();

    static PROP_STRING: Schema = ObjectSchema::new(
        "object containing array",
        &[(
            "data",
            false,
            &StringSchema::new("data")
                .format(&ApiStringFormat::PropertyString(&LIMITED_ARRAY))
                .schema(),
        )],
    )
    .schema();

    de::verify(&PROP_STRING, "data=1;2;3").expect("failed to verify valid array");
    compare_error(
        &[("data", "array must contain at most 3 elements")],
        de::verify(&PROP_STRING, "data=1;2;3;4").expect_err("max_length check failed"),
    )?;
    compare_error(
        &[("data/[1]", "not an integer: \"x\"")],
        de::verify(&PROP_STRING, "data=1;x").expect_err("invalid element check failed"),
    )?;

    de::verify(&LIMITED_ARRAY, "1,2,3").expect("failed to verify valid array");
    assert_eq!(
        de::verify(&LIMITED_ARRAY, "1;2;3;4")
            .expect_err("max_length check failed")
            .to_string(),
        "array must contain at most 3 elements"
    );

    compare_error(
        &[
            ("", "array must contain at most 3 elements"),
            ("[4]", "value must have a maximum value of 200 (got 888)"),
            ("[5]", "value must have a maximum value of 200 (got 999)"),
        ],
        de::verify(&LIMITED_ARRAY, "1;2;3;4;888;999")
            .expect_err("max_length and type check failed"),
    )?;

    Ok(())
}
