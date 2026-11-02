use std::fs::read_to_string;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::Result;

mod helper;
use helper::{Cleanup, LayerEntry, TestImage, assert_breakout_rejected};

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

#[test]
fn test_replace_symlink_with_dir() -> Result<()> {
    let mut cleanup = Cleanup::default();
    let foreign_dir = cleanup.tmp_dir("foreign dir")?;
    let foreign_mode = foreign_dir.metadata()?.permissions().mode();

    let image = TestImage::new(vec![
        vec![LayerEntry::symlink("etc/a", &foreign_dir)],
        vec![LayerEntry::dir("etc/a")],
    ])?;
    image.extract()?;

    assert!(image.extract_dir.join("etc/a").symlink_metadata()?.is_dir());
    assert_eq!(foreign_dir.metadata()?.permissions().mode(), foreign_mode);

    Ok(())
}

#[test]
fn test_replace_symlink_with_populated_dir() -> Result<()> {
    let image = TestImage::new(vec![
        vec![
            LayerEntry::dir("usr/bin"),
            LayerEntry::file("usr/bin/sh", b"sh"),
            LayerEntry::symlink("bin", "usr/bin"),
        ],
        vec![
            LayerEntry::dir("bin"),
            LayerEntry::file("bin/new", b"new"),
            LayerEntry::dir("bin/sub"),
            LayerEntry::file("bin/sub/leaf", b"leaf"),
        ],
    ])?;
    image.extract()?;

    let extract_dir = &image.extract_dir;
    assert!(extract_dir.join("bin").symlink_metadata()?.is_dir());
    assert_eq!(read_to_string(extract_dir.join("bin/new"))?, "new");
    assert_eq!(read_to_string(extract_dir.join("bin/sub/leaf"))?, "leaf");
    assert_eq!(read_to_string(extract_dir.join("usr/bin/sh"))?, "sh");
    assert!(!extract_dir.join("usr/bin/new").exists());
    assert!(!extract_dir.join("usr/bin/sub").exists());

    Ok(())
}

#[test]
fn test_replace_dir_with_symlink_in_same_layer() -> Result<()> {
    let mut cleanup = Cleanup::default();
    let foreign_dir = cleanup.tmp_dir("foreign dir")?;
    let foreign_mode = foreign_dir.metadata()?.permissions().mode();

    let image = TestImage::new(vec![vec![
        LayerEntry::dir("etc/a"),
        LayerEntry::symlink("etc/a", &foreign_dir),
    ]])?;
    image.extract()?;

    assert!(image.extract_dir.join("etc/a").is_symlink());
    assert_eq!(foreign_dir.metadata()?.permissions().mode(), foreign_mode);

    Ok(())
}

#[test]
fn test_replace_symlink_with_dir_after_its_entries() -> Result<()> {
    let image = TestImage::new(vec![
        vec![
            LayerEntry::dir("usr/bin"),
            LayerEntry::file("usr/bin/sh", b"sh"),
            LayerEntry::symlink("bin", "usr/bin"),
        ],
        vec![LayerEntry::file("bin/new", b"new"), LayerEntry::dir("bin")],
    ])?;
    image.extract()?;

    let extract_dir = &image.extract_dir;
    assert!(extract_dir.join("bin").symlink_metadata()?.is_dir());
    assert_eq!(read_to_string(extract_dir.join("bin/new"))?, "new");
    assert!(!extract_dir.join("usr/bin/new").exists());

    Ok(())
}
