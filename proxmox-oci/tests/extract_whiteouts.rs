use anyhow::Result;

mod helper;
use helper::{LayerEntry, TestImage};

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
