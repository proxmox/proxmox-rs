use std::fs::read_to_string;
use std::path::Path;

use anyhow::Result;

mod helper;
use helper::{LayerEntry, TestImage, assert_breakout_rejected};

#[test]
fn test_whiteout_root_breakout() -> Result<()> {
    let image = TestImage::new(vec![vec![LayerEntry::file(".wh..", b"")]])?;
    image.extract()?;

    // Check that the whiteout did not remove the root directory
    assert!(image.extract_dir.exists());

    Ok(())
}

#[test]
fn test_whiteout_root_parent_breakout() -> Result<()> {
    let image = TestImage::new(vec![vec![LayerEntry::file(".wh...", b"")]])?;
    image.extract()?;

    // Check that the whiteout did not remove the root directory
    assert!(image.extract_dir.exists());

    Ok(())
}

#[test]
fn test_whiteout_current_directory() -> Result<()> {
    let image = TestImage::new(vec![
        vec![LayerEntry::file("etc/passwd", b"passwd")],
        vec![LayerEntry::file("etc/.wh..", b"")],
    ])?;
    image.extract()?;

    assert!(!image.extract_dir.join("etc").exists());

    Ok(())
}

#[test]
fn test_whiteout_symlink() -> Result<()> {
    let image = TestImage::new(vec![
        vec![
            LayerEntry::file("etc/passwd", b"passwd"),
            LayerEntry::symlink("localetc", "etc"),
        ],
        vec![LayerEntry::file(".wh.localetc", b"")],
    ])?;
    image.extract()?;

    assert!(image.extract_dir.join("etc/passwd").exists());
    assert!(!image.extract_dir.join("localetc").exists());

    Ok(())
}

#[test]
fn test_whiteout_dead_symlink_parent() -> Result<()> {
    let image = TestImage::new(vec![
        vec![
            LayerEntry::file("etc/passwd", b"passwd"),
            LayerEntry::symlink("localetc", "dne"),
        ],
        vec![LayerEntry::file("localetc/.wh.etc", b"")],
    ])?;
    image.extract()?;

    assert!(image.extract_dir.join("etc/passwd").exists());

    Ok(())
}

#[test]
fn test_whiteout_opaque_foreign_dir() -> Result<()> {
    let image = assert_breakout_rejected(|foreign_dir| {
        let foreign_dir = Path::new("etc/../..").join(foreign_dir.file_name().unwrap());
        vec![
            vec![LayerEntry::file("etc/file", b"data\n")],
            vec![LayerEntry::file(foreign_dir.join(".wh..wh..opq"), b"")],
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
fn test_whiteout_dangling_symlink() -> Result<()> {
    let image = TestImage::new(vec![
        vec![LayerEntry::symlink("dangling", "dne")],
        vec![LayerEntry::file(".wh.dangling", b"")],
    ])?;
    image.extract()?;

    assert!(!image.extract_dir.join("dangling").is_symlink());

    Ok(())
}

#[test]
fn test_whiteout_opaque_with_dangling_symlink() -> Result<()> {
    let image = TestImage::new(vec![
        vec![
            LayerEntry::file("etc/passwd", b"passwd"),
            LayerEntry::symlink("etc/dangling", "dne"),
        ],
        vec![LayerEntry::file("etc/.wh..wh..opq", b"")],
    ])?;
    image.extract()?;

    assert!(image.extract_dir.join("etc").is_dir());
    assert!(!image.extract_dir.join("etc/passwd").exists());
    assert!(!image.extract_dir.join("etc/dangling").is_symlink());

    Ok(())
}

#[test]
fn test_whiteout_symlink_parent_breakout() -> Result<()> {
    assert_breakout_rejected(|foreign_dir| {
        vec![
            vec![LayerEntry::symlink("etc/link", foreign_dir)],
            vec![LayerEntry::file("etc/link/.wh.foreign.txt", b"")],
        ]
    })?;

    Ok(())
}

#[test]
fn test_whiteout_opaque_symlink_breakout() -> Result<()> {
    assert_breakout_rejected(|foreign_dir| {
        vec![
            vec![LayerEntry::symlink("etc/link", foreign_dir)],
            vec![LayerEntry::file("etc/link/.wh..wh..opq", b"")],
        ]
    })?;

    Ok(())
}
