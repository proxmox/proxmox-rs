use proxmox_sortable_macro::sortable;

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
