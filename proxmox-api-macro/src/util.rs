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
