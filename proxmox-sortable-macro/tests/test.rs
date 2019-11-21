use proxmox_sortable_macro::sortable;

// The way #[sorted] works we require an 'identity' macro due to the inability of the syntax tree
// visitor to change the type of a syntax tree element.
//
// Iow.: it replaces `sorted!([3, 2, 1])` with `identity!([1, 2, 3])`.
macro_rules! identity {
    ($($x:tt)*) => { $($x)* }
}

// In a normal project we would use this Cargo.toml line:
//
// [dependencies]
// proxmox = { version = "0.1", features = [ "sortable-macro" ] }
//
// Then:
// use proxmox::{sortable, identity};

#[test]
fn test_id() {
    #[sortable]
    const FOO: [&str; 3] = sorted!(["3", "2", "1"]);
    assert_eq!(FOO, ["1", "2", "3"]);

    #[sortable]
    const FOO2: [(&str, usize); 3] = sorted!([("3", 1), ("2", 2), ("1", 3)]);
    assert_eq!(FOO2, [("1", 3), ("2", 2), ("3", 1)]);
}
