use std::fs::read_to_string;

use anyhow::Result;

mod helper;
use helper::{LayerEntry, TestImage};

#[test]
fn test_replace_file() -> Result<()> {
    let image = TestImage::new(vec![
        vec![LayerEntry::file("etc/a", b"1")],
        vec![LayerEntry::file("etc/a", b"2")],
    ])?;
    image.extract()?;

    let replaced_path = image.extract_dir.join("etc/a");
    assert!(replaced_path.is_file());
    assert_eq!(read_to_string(replaced_path)?, "2");

    Ok(())
}

#[test]
fn test_replace_file_with_dir() -> Result<()> {
    let image = TestImage::new(vec![
        vec![LayerEntry::file("etc/a", b"1")],
        vec![LayerEntry::dir("etc/a")],
    ])?;
    image.extract()?;

    assert!(image.extract_dir.join("etc/a").is_dir());

    Ok(())
}

#[test]
fn test_replace_dir_with_file() -> Result<()> {
    let image = TestImage::new(vec![
        vec![LayerEntry::dir("etc/a")],
        vec![LayerEntry::file("etc/a", b"1")],
    ])?;
    image.extract()?;

    assert!(image.extract_dir.join("etc/a").is_file());

    Ok(())
}
