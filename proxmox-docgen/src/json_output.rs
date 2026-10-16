use proxmox_schema::Schema;
use proxmox_schema::format::wrap_text;

/// The JSON type name of `schema`, as it appears in the output.
///
/// An array names its element type as well, for example `array of object`. `AllOf` and `OneOf`
/// are plain objects here, because that is what they serialize to.
///
/// This deliberately differs from [`proxmox_schema::format::get_schema_type_text`]. That one
/// describes what a user has to type on a command line, down to enum variants and value ranges,
/// and names the element type of an array because such an option is repeated rather than passed
/// a list. Here the value has been printed already, so its JSON type is what matters.
pub fn json_type_text(schema: &Schema) -> String {
    match schema {
        Schema::Null => "null".to_string(),
        Schema::Boolean(_) => "boolean".to_string(),
        Schema::Integer(_) => "integer".to_string(),
        Schema::Number(_) => "number".to_string(),
        Schema::String(_) => "string".to_string(),
        Schema::Object(_) | Schema::AllOf(_) | Schema::OneOf(_) => "object".to_string(),
        Schema::Array(array) => format!("array of {}", json_type_text(array.items)),
    }
}

/// The description of any schema variant, which `Schema` itself does not expose.
fn description_of(schema: &Schema) -> &'static str {
    match schema {
        Schema::Null => "",
        Schema::Boolean(s) => s.description,
        Schema::Integer(s) => s.description,
        Schema::Number(s) => s.description,
        Schema::String(s) => s.description,
        Schema::Object(s) => s.description,
        Schema::Array(s) => s.description,
        Schema::AllOf(s) => s.description,
        Schema::OneOf(s) => s.description,
    }
}

/// Generate ReST documentation for the JSON output that `schema` describes.
///
/// Renders the schema's own description, then one definition list entry per property in the
/// order the schema defines them. Optional properties are marked inline instead of being
/// collected in a section of their own, which keeps that order intact.
///
/// Nested types are named but not expanded: a property is rendered as `object` or as
/// `array of object` and gets no sub-list. Give such a type a heading and a call of its own.
/// That keeps the result readable at any depth and terminates on schemas that refer back to
/// themselves.
///
/// Every `OneOf` gets one section per variant, at whatever depth it appears. The sections follow
/// all the properties the object always has, whichever order an `AllOf` lists its members in,
/// see `dump_json_object`.
///
/// Use [`proxmox_schema::format::dump_properties`] instead for parameters and configuration
/// keys, where the text has to describe what a user may type.
///
/// # Panics
///
/// If `schema` is not an object, an `AllOf` or a `OneOf`.
pub fn dump_json_properties(schema: &Schema) -> String {
    let mut res = String::new();

    let description = description_of(schema);
    if !description.is_empty() {
        res.push_str(&wrap_text("", "", description, 80));
        res.push_str("\n\n");
    }

    res.push_str(&dump_json_object(schema, &mut Vec::new()));

    res
}

/// Render the object `schema` describes: the properties every printed object has, then a section
/// per alternative.
///
/// `seen` holds the names rendered so far and gets the ones rendered here added, so that a
/// property two members of an `AllOf` both define is listed once.
fn dump_json_object(schema: &Schema, seen: &mut Vec<&'static str>) -> String {
    let mut res = dump_json_common(schema, seen);
    res.push_str(&dump_json_alternatives(schema, seen));
    res
}

/// Render the properties that every object `schema` describes has.
///
/// An `AllOf` contributes the properties of all its members, as all of them apply at once. A
/// `OneOf` contributes only its type property, which every variant has. The properties of the
/// variants are rendered by [`dump_json_alternatives`] instead, after this pass is complete for
/// the whole schema. Rendering them in member order would let an `AllOf` that lists a `OneOf`
/// before a plain object make the object's properties read as part of the last variant.
fn dump_json_common(schema: &Schema, seen: &mut Vec<&'static str>) -> String {
    match schema {
        Schema::Object(object) => {
            let mut res = String::new();

            for (name, optional, schema) in object.properties {
                if seen.contains(name) {
                    continue;
                }
                seen.push(name);
                res.push_str(&dump_property(name, *optional, schema));
            }

            res
        }
        Schema::AllOf(all_of) => all_of
            .list
            .iter()
            .map(|schema| dump_json_common(schema, seen))
            .collect(),
        Schema::OneOf(one_of) => {
            let &(type_name, type_optional, type_schema) = one_of.type_property_entry;
            if seen.contains(&type_name) {
                return String::new();
            }
            seen.push(type_name);
            dump_property(type_name, type_optional, type_schema)
        }
        _ => panic!("dump_json_properties on a schema that is not an object"),
    }
}

/// Render one section per variant of every `OneOf` in `schema`.
///
/// The variants are alternatives rather than parts of one object: merged into a single list a
/// property that only one variant defines would read as always present, and a name that two
/// variants give different types would show only the first of them. Each section is a complete
/// object of its own, so a variant that holds another `OneOf` gets nested sections.
fn dump_json_alternatives(schema: &Schema, seen: &[&'static str]) -> String {
    match schema {
        Schema::Object(_) => String::new(),
        Schema::AllOf(all_of) => all_of
            .list
            .iter()
            .map(|schema| dump_json_alternatives(schema, seen))
            .collect(),
        Schema::OneOf(one_of) => {
            let type_name = one_of.type_property();
            let mut res = String::new();

            for (value, variant) in one_of.list {
                res.push_str(&format!("\nWhen ``{type_name}`` is ``{value}``:\n\n"));

                let description = description_of(variant);
                if !description.is_empty() {
                    res.push_str(&wrap_text("  ", "  ", description, 80));
                    res.push_str("\n\n");
                }

                // a name one variant uses must not keep another from documenting its own, so
                // each of them starts from the names the object always has
                let body = dump_json_object(variant, &mut seen.to_vec());
                for line in body.lines() {
                    if !line.is_empty() {
                        res.push_str("  ");
                        res.push_str(line);
                    }
                    res.push('\n');
                }
            }

            res
        }
        _ => panic!("dump_json_properties on a schema that is not an object"),
    }
}

/// Render a single definition list entry.
fn dump_property(name: &str, optional: bool, schema: &Schema) -> String {
    let optional = if optional { ", optional" } else { "" };
    let type_text = json_type_text(schema);
    let mut res = format!("``{name}`` : ``{type_text}``{optional}\n");

    let description = description_of(schema);
    if !description.trim().is_empty() {
        res.push_str(&wrap_text("  ", "  ", description, 80));
        res.push('\n');
    } else {
        // An empty comment supplies the definition body required by ReST without visible text.
        res.push_str("  ..\n");
    }

    res
}

#[cfg(test)]
mod tests {
    use proxmox_schema::{
        AllOfSchema, ArraySchema, BooleanSchema, IntegerSchema, ObjectSchema, OneOfSchema, Schema,
        StringSchema,
    };

    use super::{dump_json_properties, json_type_text};

    const NAME_SCHEMA: Schema = StringSchema::new("The name.").schema();
    const ITEM_SCHEMA: Schema =
        ObjectSchema::new("One item.", &[("name", false, &NAME_SCHEMA)]).schema();
    const ITEMS_SCHEMA: Schema =
        ArraySchema::new("The items, oldest first.", &ITEM_SCHEMA).schema();
    const COUNT_SCHEMA: Schema = IntegerSchema::new("How many there are.")
        .minimum(0)
        .schema();
    const FLAG_SCHEMA: Schema = BooleanSchema::new("Whether it happened.").schema();
    const SUMMARY_SCHEMA: Schema = ObjectSchema::new(
        "A summary.",
        &[
            ("count", true, &COUNT_SCHEMA),
            ("flag", false, &FLAG_SCHEMA),
            ("items", false, &ITEMS_SCHEMA),
        ],
    )
    .schema();

    const KIND_SCHEMA: Schema = StringSchema::new("Which kind it is.").schema();
    const FIRST_SCHEMA: Schema =
        ObjectSchema::new("The first kind.", &[("count", false, &COUNT_SCHEMA)]).schema();
    const SECOND_SCHEMA: Schema =
        ObjectSchema::new("The second kind.", &[("name", false, &NAME_SCHEMA)]).schema();
    const THING_SCHEMA: Schema = OneOfSchema::new(
        "A thing.",
        &("kind", false, &KIND_SCHEMA),
        &[("first", &FIRST_SCHEMA), ("second", &SECOND_SCHEMA)],
    )
    .schema();

    const COMMON_SCHEMA: Schema =
        ObjectSchema::new("The common part.", &[("common", false, &NAME_SCHEMA)]).schema();
    const COMBINED_SCHEMA: Schema = AllOfSchema::new(
        "Common fields plus one alternative.",
        &[&COMMON_SCHEMA, &THING_SCHEMA],
    )
    .schema();

    #[test]
    fn json_type_text_names_the_element_type_of_an_array() {
        assert_eq!(json_type_text(&FLAG_SCHEMA), "boolean");
        assert_eq!(json_type_text(&COUNT_SCHEMA), "integer");
        assert_eq!(json_type_text(&NAME_SCHEMA), "string");
        assert_eq!(json_type_text(&ITEM_SCHEMA), "object");
        // the constraints get_schema_type_text would add belong to a command line, not to
        // JSON output, and a list must not be rendered as its element type
        assert_eq!(json_type_text(&ITEMS_SCHEMA), "array of object");
    }

    #[test]
    fn dump_json_properties_keeps_schema_order_and_marks_optional_inline() {
        let dumped = dump_json_properties(&SUMMARY_SCHEMA);
        assert_eq!(
            dumped,
            "A summary.\n\
             \n\
             ``count`` : ``integer``, optional\n\
             \x20 How many there are.\n\
             ``flag`` : ``boolean``\n\
             \x20 Whether it happened.\n\
             ``items`` : ``array of object``\n\
             \x20 The items, oldest first.\n",
        );
    }

    #[test]
    fn dump_json_properties_gives_each_one_of_variant_its_own_section() {
        let dumped = dump_json_properties(&THING_SCHEMA);
        assert_eq!(
            dumped,
            "A thing.\n\
             \n\
             ``kind`` : ``string``\n\
             \x20 Which kind it is.\n\
             \n\
             When ``kind`` is ``first``:\n\
             \n\
             \x20 The first kind.\n\
             \n\
             \x20 ``count`` : ``integer``\n\
             \x20\x20\x20 How many there are.\n\
             \n\
             When ``kind`` is ``second``:\n\
             \n\
             \x20 The second kind.\n\
             \n\
             \x20 ``name`` : ``string``\n\
             \x20\x20\x20 The name.\n",
        );

        // a property of one variant is not part of every printed object, so it must not show up
        // next to the properties of the other one as if it always were
        assert!(!dumped.contains("``count`` : ``integer``\n  How many there are.\n``name``"));
    }

    #[test]
    fn dump_json_properties_keeps_the_type_of_each_one_of_variant() {
        const AS_COUNT: Schema =
            ObjectSchema::new("Counted.", &[("value", false, &COUNT_SCHEMA)]).schema();
        const AS_NAME: Schema =
            ObjectSchema::new("Named.", &[("value", false, &NAME_SCHEMA)]).schema();
        const EITHER_SCHEMA: Schema = OneOfSchema::new(
            "Either way.",
            &("kind", false, &KIND_SCHEMA),
            &[("counted", &AS_COUNT), ("named", &AS_NAME)],
        )
        .schema();

        let dumped = dump_json_properties(&EITHER_SCHEMA);

        // the same name has a different type per variant, both of them have to survive
        assert!(dumped.contains("``value`` : ``integer``"), "{dumped}");
        assert!(dumped.contains("``value`` : ``string``"), "{dumped}");
    }

    #[test]
    fn dump_json_properties_sections_a_one_of_inside_an_all_of() {
        let dumped = dump_json_properties(&COMBINED_SCHEMA);
        assert_eq!(
            dumped,
            "Common fields plus one alternative.\n\
             \n\
             ``common`` : ``string``\n\
             \x20 The name.\n\
             ``kind`` : ``string``\n\
             \x20 Which kind it is.\n\
             \n\
             When ``kind`` is ``first``:\n\
             \n\
             \x20 The first kind.\n\
             \n\
             \x20 ``count`` : ``integer``\n\
             \x20\x20\x20 How many there are.\n\
             \n\
             When ``kind`` is ``second``:\n\
             \n\
             \x20 The second kind.\n\
             \n\
             \x20 ``name`` : ``string``\n\
             \x20\x20\x20 The name.\n",
        );
    }

    #[test]
    fn dump_json_properties_lists_common_properties_before_any_variant() {
        const REVERSED_SCHEMA: Schema = AllOfSchema::new(
            "One alternative plus common fields.",
            &[&THING_SCHEMA, &COMMON_SCHEMA],
        )
        .schema();

        // the member order of an all-of must not decide whether a property reads as always
        // present or as part of the last variant
        let dumped = dump_json_properties(&REVERSED_SCHEMA);
        assert_eq!(
            dumped,
            "One alternative plus common fields.\n\
             \n\
             ``kind`` : ``string``\n\
             \x20 Which kind it is.\n\
             ``common`` : ``string``\n\
             \x20 The name.\n\
             \n\
             When ``kind`` is ``first``:\n\
             \n\
             \x20 The first kind.\n\
             \n\
             \x20 ``count`` : ``integer``\n\
             \x20\x20\x20 How many there are.\n\
             \n\
             When ``kind`` is ``second``:\n\
             \n\
             \x20 The second kind.\n\
             \n\
             \x20 ``name`` : ``string``\n\
             \x20\x20\x20 The name.\n",
        );
    }

    #[test]
    fn empty_descriptions_have_a_definition_body() {
        const EMPTY_DESCRIPTION: Schema = StringSchema::new(" \n ").schema();
        const SCHEMA: Schema = ObjectSchema::new(
            "",
            &[
                ("absent", true, &Schema::Null),
                ("empty", false, &EMPTY_DESCRIPTION),
                ("name", false, &NAME_SCHEMA),
            ],
        )
        .schema();

        assert_eq!(
            dump_json_properties(&SCHEMA),
            "``absent`` : ``null``, optional\n\
             \x20 ..\n\
             ``empty`` : ``string``\n\
             \x20 ..\n\
             ``name`` : ``string``\n\
             \x20 The name.\n",
        );
    }

    #[test]
    fn dump_json_properties_sections_a_one_of_inside_a_one_of() {
        const INNER_SCHEMA: Schema = OneOfSchema::new(
            "The inner choice.",
            &("sub", false, &KIND_SCHEMA),
            &[("left", &FIRST_SCHEMA), ("right", &SECOND_SCHEMA)],
        )
        .schema();
        const OUTER_SCHEMA: Schema = OneOfSchema::new(
            "The outer choice.",
            &("kind", false, &KIND_SCHEMA),
            &[("nested", &INNER_SCHEMA), ("plain", &FIRST_SCHEMA)],
        )
        .schema();

        let dumped = dump_json_properties(&OUTER_SCHEMA);

        assert_eq!(
            dumped,
            "The outer choice.\n\
             \n\
             ``kind`` : ``string``\n\
             \x20 Which kind it is.\n\
             \n\
             When ``kind`` is ``nested``:\n\
             \n\
             \x20 The inner choice.\n\
             \n\
             \x20 ``sub`` : ``string``\n\
             \x20\x20\x20 Which kind it is.\n\
             \n\
             \x20 When ``sub`` is ``left``:\n\
             \n\
             \x20\x20\x20 The first kind.\n\
             \n\
             \x20\x20\x20 ``count`` : ``integer``\n\
             \x20\x20\x20\x20\x20 How many there are.\n\
             \n\
             \x20 When ``sub`` is ``right``:\n\
             \n\
             \x20\x20\x20 The second kind.\n\
             \n\
             \x20\x20\x20 ``name`` : ``string``\n\
             \x20\x20\x20\x20\x20 The name.\n\
             \n\
             When ``kind`` is ``plain``:\n\
             \n\
             \x20 The first kind.\n\
             \n\
             \x20 ``count`` : ``integer``\n\
             \x20\x20\x20 How many there are.\n",
        );
    }
}
