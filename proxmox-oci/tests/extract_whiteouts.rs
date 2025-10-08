use std::fs::remove_dir_all;

use proxmox_oci::{parse_and_extract_image, Arch};
use proxmox_sys::fs::make_tmp_dir;

#[test]
fn test_whiteout_root_breakout() {
    let extract_dir = make_tmp_dir("/tmp/", None).unwrap();

    parse_and_extract_image(
        &"tests/oci_image_data/oci_test_whiteout_root_breakout.tar".into(),
        &extract_dir,
        Some(&Arch::Amd64),
    )
    .unwrap();

    // Check that the whiteout did not remove the root directory
    assert!(extract_dir.exists());

    // Cleanup
    remove_dir_all(extract_dir).unwrap();
}

#[test]
fn test_whiteout_root_parent_breakout() {
    let extract_dir = make_tmp_dir("/tmp/", None).unwrap();

    parse_and_extract_image(
        &"tests/oci_image_data/oci_test_whiteout_root_parent_breakout.tar".into(),
        &extract_dir,
        Some(&Arch::Amd64),
    )
    .unwrap();

    // Check that the whiteout did not remove the root directory
    assert!(extract_dir.exists());

    // Cleanup
    remove_dir_all(extract_dir).unwrap();
}

#[test]
fn test_whiteout_current_directory() {
    let extract_dir = make_tmp_dir("/tmp/", None).unwrap();

    parse_and_extract_image(
        &"tests/oci_image_data/oci_test_whiteout_current_directory.tar".into(),
        &extract_dir,
        Some(&Arch::Amd64),
    )
    .unwrap();

    assert!(!extract_dir.join("etc").exists());

    // Cleanup
    remove_dir_all(extract_dir).unwrap();
}

#[test]
fn test_whiteout_symlink() {
    let extract_dir = make_tmp_dir("/tmp/", None).unwrap();

    parse_and_extract_image(
        &"tests/oci_image_data/oci_test_whiteout_symlink.tar".into(),
        &extract_dir,
        Some(&Arch::Amd64),
    )
    .unwrap();

    assert!(extract_dir.join("etc/passwd").exists());
    assert!(!extract_dir.join("localetc").exists());

    // Cleanup
    remove_dir_all(extract_dir).unwrap();
}

#[test]
fn test_whiteout_dead_symlink_parent() {
    let extract_dir = make_tmp_dir("/tmp/", None).unwrap();

    parse_and_extract_image(
        &"tests/oci_image_data/oci_test_whiteout_dead_symlink_parent.tar".into(),
        &extract_dir,
        Some(&Arch::Amd64),
    )
    .unwrap();

    assert!(extract_dir.join("etc/passwd").exists());

    // Cleanup
    remove_dir_all(extract_dir).unwrap();
}
