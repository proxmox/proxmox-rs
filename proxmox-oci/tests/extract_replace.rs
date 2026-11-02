use std::fs::read_to_string;
use std::path::Path;

use anyhow::Result;

mod helper;
use helper::{LayerEntry, TestImage, assert_breakout_rejected};

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

#[test]
fn test_replace_foreign_file() -> Result<()> {
    let image = assert_breakout_rejected(|foreign_dir| {
        let foreign_dir = Path::new("etc/../..").join(foreign_dir.file_name().unwrap());
        vec![
            vec![LayerEntry::file("etc/file", b"data\n")],
            // A symlink, as whiteout handling would already reject a regular file.
            vec![LayerEntry::symlink(
                foreign_dir.join("foreign.txt"),
                "replaced",
            )],
            vec![LayerEntry::file("etc/after", b"after\n")],
        ]
    })?;

    assert_eq!(
        read_to_string(image.extract_dir.join("etc/file"))?,
        "data\n"
    );
    assert!(!image.extract_dir.join("etc/after").exists());

    Ok(())
}

#[test]
fn test_replace_file_through_symlink_parent() -> Result<()> {
    assert_breakout_rejected(|foreign_dir| {
        vec![
            vec![LayerEntry::symlink("etc/link", foreign_dir)],
            vec![LayerEntry::file("etc/link/foreign.txt", b"replaced\n")],
        ]
    })?;

    Ok(())
}

#[test]
fn test_replace_file_with_dir_through_symlink_parent() -> Result<()> {
    assert_breakout_rejected(|foreign_dir| {
        vec![
            vec![LayerEntry::symlink("etc/link", foreign_dir)],
            vec![LayerEntry::dir("etc/link/foreign.txt")],
        ]
    })?;

    Ok(())
}
