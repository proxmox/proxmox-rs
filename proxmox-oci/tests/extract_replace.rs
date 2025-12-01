use std::fs::{read_to_string, remove_dir_all};

use proxmox_oci::{parse_and_extract_image, Arch};
use proxmox_sys::fs::make_tmp_dir;

#[test]
fn test_replace_file() {
    let extract_dir = make_tmp_dir("/tmp/", None).unwrap();

    parse_and_extract_image(
        &"tests/oci_image_data/oci_test_replace_file.tar".into(),
        &extract_dir,
        Some(&Arch::Amd64),
    )
    .unwrap();

    let replaced_path = extract_dir.join("etc/a");
    assert!(replaced_path.is_file());
    assert_eq!(read_to_string(replaced_path).unwrap(), "2");

    // Cleanup
    remove_dir_all(extract_dir).unwrap();
}

#[test]
fn test_replace_file_with_dir() {
    let extract_dir = make_tmp_dir("/tmp/", None).unwrap();

    parse_and_extract_image(
        &"tests/oci_image_data/oci_test_replace_file_with_dir.tar".into(),
        &extract_dir,
        Some(&Arch::Amd64),
    )
    .unwrap();

    assert!(extract_dir.join("etc/a").is_dir());

    // Cleanup
    remove_dir_all(extract_dir).unwrap();
}

#[test]
fn test_replace_dir_with_file() {
    let extract_dir = make_tmp_dir("/tmp/", None).unwrap();

    parse_and_extract_image(
        &"tests/oci_image_data/oci_test_replace_dir_with_file.tar".into(),
        &extract_dir,
        Some(&Arch::Amd64),
    )
    .unwrap();

    assert!(extract_dir.join("etc/a").is_file());

    // Cleanup
    remove_dir_all(extract_dir).unwrap();
}
