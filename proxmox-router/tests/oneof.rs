#![cfg(feature = "cli")]

use anyhow::Error;
use proxmox_router::cli::*;
use proxmox_router::{ApiHandler, ApiMethod, RpcEnvironment};
use proxmox_schema::format::DocumentationFormat;
use proxmox_schema::*;
use serde::Deserialize;
use serde_json::{Value, json};

static STRING: Schema = StringSchema::new("A string.").schema();
static KIND: Schema = StringSchema::new("The variant.")
    .format(&ApiStringFormat::Enum(&[
        EnumEntry::new("a", "Variant A."),
        EnumEntry::new("b", "Variant B."),
    ]))
    .schema();
static A: Schema = ObjectSchema::new(
    "A.",
    &[("alpha", true, &STRING), ("comment", true, &STRING)],
)
.schema();
static B: Schema =
    ObjectSchema::new("B.", &[("beta", true, &STRING), ("comment", true, &STRING)]).schema();
static ENUM: Schema =
    OneOfSchema::new("Choice.", &("kind", false, &KIND), &[("a", &A), ("b", &B)]).schema();
static PARTS: &[&Schema] = &[&ENUM];
static ALL: Schema = AllOfSchema::new("Flattened choice.", PARTS).schema();

fn handler(_: Value, _: &ApiMethod, _: &mut dyn RpcEnvironment) -> Result<Value, Error> {
    Ok(Value::Null)
}

static METHOD: ApiMethod = ApiMethod::new_full(
    &ApiHandler::Sync(&handler),
    ParameterSchema::AllOf(ALL.unwrap_all_of_schema()),
);
static EMPTY: ApiMethod = ApiMethod::new(
    &ApiHandler::Sync(&handler),
    &ObjectSchema::new("No parameters.", &[]),
);

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "kind")]
enum Choice {
    #[serde(rename = "a")]
    A {
        alpha: Option<String>,
        comment: Option<String>,
    },
    #[serde(rename = "b")]
    B {
        beta: Option<String>,
        comment: Option<String>,
    },
}
#[derive(Deserialize)]
struct Options {
    #[serde(flatten)]
    choice: Choice,
}
impl ApiType for Options {
    const API_SCHEMA: Schema = AllOfSchema::new("Flattened options.", PARTS).schema();
}

#[test]
fn global_options_follow_the_active_variant() {
    let def = CliCommandMap::new()
        .global_option(GlobalOptions::of::<Options>())
        .insert("run", CliCommand::new(&EMPTY))
        .build();

    // `comment` exists in both variants, but is a single option
    let completions = def.get_completions("cmd --", true).1;
    assert_eq!(
        completions.iter().filter(|c| *c == "--comment").count(),
        1,
        "{completions:?}"
    );
    let mut help = String::new();
    print_help_to(&def, "cmd".into(), &[], Some(false), &mut help).unwrap();
    assert_eq!(help.matches("--comment").count(), 1, "{help}");

    let cli = CommandLine::new(def);
    let parse = |args: &[&str]| -> Result<Choice, Error> {
        let mut env = CliEnvironment::new();
        cli.parse(&mut env, args.iter().map(|arg| arg.to_string()))?;
        Ok(env.take_global_option::<Options>().unwrap().choice)
    };
    assert_eq!(
        parse(&[
            "cmd",
            "--kind",
            "a",
            "--alpha",
            "x",
            "--comment",
            "y",
            "run"
        ])
        .unwrap(),
        Choice::A {
            alpha: Some("x".into()),
            comment: Some("y".into()),
        }
    );
    // options of the inactive variant are rejected rather than ignored
    assert!(parse(&["cmd", "--kind", "a", "--beta", "x", "run"]).is_err());
}

fn format_object(schema: &Schema, mut data: Value) -> String {
    let mut out = Vec::new();
    value_to_text(
        &mut out,
        &mut data,
        schema,
        &TableFormatOptions::default().noborder(true),
    )
    .unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn object_tables_only_include_active_fields() {
    let output = format_object(&ALL, json!({"kind": "a", "alpha": "hello"}));
    assert!(
        output.contains("alpha") && !output.contains("beta"),
        "{output}"
    );

    // also for variants which are not plain objects themselves
    static OUTER: Schema = OneOfSchema::new(
        "Nested choice.",
        &("outer", false, &STRING),
        &[("all", &ALL), ("one", &ENUM)],
    )
    .schema();
    for outer in ["all", "one"] {
        let output = format_object(
            &OUTER,
            json!({"outer": outer, "kind": "a", "alpha": "hello"}),
        );
        assert!(
            output.contains("alpha") && !output.contains("beta"),
            "{output}"
        );
    }
}

#[test]
fn table_columns_and_default_sort_follow_schema_order() {
    static OBJECT: Schema = ObjectSchema::new(
        "Ordinary object.",
        &[
            ("alpha", false, &STRING),
            ("beta", false, &STRING),
            ("gamma", true, &STRING),
        ],
    )
    .schema();
    static ARRAY: Schema = ArraySchema::new("Objects.", &OBJECT).schema();
    // repeat, as the order would vary between hash maps
    for _ in 0..16 {
        let mut data = json!([
            {"alpha": "z", "beta": "a", "gamma": "2"},
            {"alpha": "a", "beta": "z", "gamma": "1"}
        ]);
        let mut out = Vec::new();
        value_to_text(
            &mut out,
            &mut data,
            &ARRAY,
            &TableFormatOptions::default().noborder(true),
        )
        .unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "a z 1\nz a 2\n");
    }
}

#[test]
fn shared_columns_are_required_if_any_variant_requires_them() {
    static OPTIONAL: Schema = ObjectSchema::new("Optional.", &[("shared", true, &STRING)]).schema();
    static REQUIRED: Schema =
        ObjectSchema::new("Required.", &[("shared", false, &STRING)]).schema();
    static CHOICE: Schema = OneOfSchema::new(
        "Choice.",
        &("kind", false, &KIND),
        &[("a", &OPTIONAL), ("b", &REQUIRED)],
    )
    .schema();
    static COMMON: Schema = ObjectSchema::new("Common.", &[("after", false, &STRING)]).schema();
    static ROW: Schema = AllOfSchema::new("Row.", &[&CHOICE, &COMMON]).schema();
    static ARRAY: Schema = ArraySchema::new("Rows.", &ROW).schema();
    // required columns come first, so `shared` precedes `after` only if it counts as required
    let mut data = json!([{"kind": "b", "shared": "value", "after": "last"}]);
    let mut out = Vec::new();
    value_to_text(
        &mut out,
        &mut data,
        &ARRAY,
        &TableFormatOptions::default().noborder(true),
    )
    .unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), "b value last\n");
}

#[test]
fn positional_variant_fields_have_usage() {
    // `alpha` only exists in variant `a`
    let cmd = CliCommand::new(&METHOD).arg_param(&["alpha"]);
    let text = generate_usage_str("cmd", &cmd, DocumentationFormat::Full, "", &[]);
    assert!(text.contains("[<alpha>]"), "{text}");
    assert!(
        text.contains("<alpha>    <string>\n             (if kind=\"a\")"),
        "{text}"
    );
}

#[test]
fn usage_describes_each_variant_of_a_shared_property() {
    static NAMED: Schema = ObjectSchema::new("Named.", &[("name", false, &STRING)]).schema();
    static CHOICE: Schema = OneOfSchema::new(
        "Choice.",
        &("kind", false, &KIND),
        &[("a", &NAMED), ("b", &NAMED)],
    )
    .schema();
    static METHOD: ApiMethod = ApiMethod::new_full(
        &ApiHandler::Sync(&handler),
        ParameterSchema::OneOf(CHOICE.unwrap_one_of_schema()),
    );

    for (cmd, synopsis) in [
        (CliCommand::new(&METHOD), "cmd --kind a|b --name <string>"),
        (
            CliCommand::new(&METHOD).arg_param(&["name"]),
            "cmd <name> --kind a|b",
        ),
    ] {
        assert_eq!(
            generate_usage_str("cmd", &cmd, DocumentationFormat::Short, "", &[]),
            synopsis
        );
        let text = generate_usage_str("cmd", &cmd, DocumentationFormat::Full, "", &[]);
        assert!(
            text.contains("(if kind=\"a\")") && text.contains("(if kind=\"b\")"),
            "{text}"
        );
    }
}

fn completions(method: &'static ApiMethod, line: &str) -> Vec<String> {
    let cli = CommandLineInterface::from(CliCommand::new(method));
    cli.get_completions(line, true).1
}

#[test]
fn option_completion_follows_known_types() {
    assert_eq!(
        completions(&METHOD, "cmd --"),
        ["--kind", "--alpha", "--comment", "--beta"]
    );
    assert_eq!(
        completions(&METHOD, "cmd --kind b --"),
        ["--beta", "--comment"]
    );
}

#[test]
fn value_completion_follows_known_types() {
    static FAST: Schema = StringSchema::new("Fast.")
        .format(&ApiStringFormat::Enum(&[
            EnumEntry::new("fast", "Fast mode."),
            EnumEntry::new("shared", "Shared mode."),
        ]))
        .schema();
    static SLOW: Schema = StringSchema::new("Slow.")
        .format(&ApiStringFormat::Enum(&[
            EnumEntry::new("slow", "Slow mode."),
            EnumEntry::new("shared", "Shared mode."),
        ]))
        .schema();
    static CHOICE: Schema = OneOfSchema::new(
        "Choice.",
        &("kind", false, &KIND),
        &[
            (
                "a",
                &ObjectSchema::new("A.", &[("mode", false, &FAST)]).schema(),
            ),
            (
                "b",
                &ObjectSchema::new("B.", &[("mode", false, &SLOW)]).schema(),
            ),
        ],
    )
    .schema();
    static METHOD: ApiMethod = ApiMethod::new_full(
        &ApiHandler::Sync(&handler),
        ParameterSchema::OneOf(CHOICE.unwrap_one_of_schema()),
    );
    assert_eq!(
        completions(&METHOD, "cmd --mode "),
        ["fast", "shared", "slow"]
    );
    assert_eq!(
        completions(&METHOD, "cmd --kind b --mode "),
        ["slow", "shared"]
    );
}
