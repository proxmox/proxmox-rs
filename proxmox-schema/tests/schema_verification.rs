use anyhow::{bail, Error};
use serde_json::{json, Value};

use proxmox_schema::*;

static STRING_SCHEMA: Schema = StringSchema::new("A test string").schema();

static SIMPLE_OBJECT_SCHEMA: Schema =  ObjectSchema::new(
    "simple object schema",
    &[
        ("prop1", false, &STRING_SCHEMA),
        ("prop2", true, &STRING_SCHEMA),
        ("prop3", false, &STRING_SCHEMA),
    ]
).schema();

static SIMPLE_PROPERTY_STRING_SCHEMA: Schema = StringSchema::new("simple property string")
    .format(&ApiStringFormat::PropertyString(&SIMPLE_OBJECT_SCHEMA))
    .schema();

static SIMPLE_ARRAY_SCHEMA: Schema = ArraySchema::new("String list.", &STRING_SCHEMA).schema();

static NESTED_OBJECT_SCHEMA: Schema =  ObjectSchema::new(
    "nested object schema",
    &[
        ("arr1", false, &SIMPLE_ARRAY_SCHEMA),
        ("obj1", false, &SIMPLE_OBJECT_SCHEMA),
        ("prop1", false, &STRING_SCHEMA),
    ]
).schema();

static NESTED_PROPERTY_SCHEMA: Schema =  ObjectSchema::new(
    "object with property strings",
    &[
        ("ps1", false, &SIMPLE_PROPERTY_STRING_SCHEMA),
    ]
).schema();


fn compare_error(
    expected: &[(&str, &str)],
    err: Error,
) -> Result<(), Error> {
    let err = match err.downcast_ref::<ParameterError>() {
        Some(err) => err,
        None => bail!("unable to downcast error: {}", err),
    };

    let result = (move || {
        let errors = err.errors();

        if errors.len() != expected.len() {
            bail!("error list has different length: {} != {}", expected.len(), errors.len());
        }

        for i in 0..expected.len() {
            if expected[i].0 != errors[i].0 {
                bail!("error {} path differs: '{}' != '{}'", i, expected[i].0, errors[i].0);
            }
            if expected[i].1 != errors[i].1.to_string() {
                bail!("error {} message differs: '{}' != '{}'", i, expected[i].1, errors[i].1);
            }

        }

        Ok(())
    })();

    if result.is_err() {
        println!("GOT: {:?}", err);
    }

    result
}

fn test_verify(
    schema: &Schema,
    data: &Value,
    expected_errors: &[(&str, &str)],
) -> Result<(), Error> {
    match verify_json(data, schema) {
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
            ("prop4", "schema does not allow additional properties."),
            ("prop3", "property is missing and it is not optional."),
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
            ("prop4", "schema does not allow additional properties."),
            ("arr1", "property is missing and it is not optional."),
            ("obj1", "property is missing and it is not optional."),
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
            ("obj1/prop1", "property is missing and it is not optional."),
            ("obj1/prop3", "property is missing and it is not optional."),
            ("prop1", "Expected string value."),
            ("prop4", "schema does not allow additional properties."),
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
            ("ps1", "Value without key, but schema does not define a default key."),
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
            ("ps1/abc", "schema does not allow additional properties."),
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
            ("ps1/prop1", "parameter is missing and it is not optional."),
            ("ps1/prop3", "parameter is missing and it is not optional."),
         ],
    )?;

    Ok(())
}
