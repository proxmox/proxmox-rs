use anyhow::bail;
use serde_json::Value;
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

    parse_parameter_strings(&param_list, schema.into(), test_required)
}

#[test]
fn test_schema1() {
    let schema = Schema::Object(ObjectSchema {
        description: "TEST",
        additional_properties: false,
        properties: &[],
        default_key: None,
    });

    println!("TEST Schema: {:?}", schema);
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
            ArraySchema::new("Integer List.", &IntegerSchema::new("Soemething").schema()).schema();

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
            ArraySchema::new("Integer List.", &IntegerSchema::new("Soemething").schema())
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
