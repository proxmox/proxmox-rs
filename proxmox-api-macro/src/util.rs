/// Convert `this_kind_of_text` to `ThisKindOfText`.
pub fn to_camel_case(text: &str) -> String {
    let mut out = String::new();

    let mut capitalize = true;
    for c in text.chars() {
        if c == '_' {
            capitalize = true;
        } else {
            if capitalize {
                out.extend(c.to_uppercase());
                capitalize = false;
            } else {
                out.push(c);
            }
        }
    }

    out
}

/// Convert `ThisKindOfText` to `this_kind_of_text`.
pub fn to_underscore_case(text: &str) -> String {
    let mut out = String::new();

    for c in text.chars() {
        if c.is_uppercase() {
            if !out.is_empty() {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }

    out
}
