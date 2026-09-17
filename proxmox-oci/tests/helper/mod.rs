#![allow(dead_code)]

use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use anyhow::Result;

use proxmox_oci::{Arch, Config, ProxmoxOciError, parse_and_extract_image};
use proxmox_sys::fs::make_tmp_dir;

mod oci_tar_image_builder;
pub use oci_tar_image_builder::{LayerEntry, OciTarImageBuilder};

#[derive(Default)]
pub struct Cleanup {
    dirs: Vec<(PathBuf, &'static str)>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        for (dir, note) in &self.dirs {
            eprintln!("Cleanup dir {dir:?} ({note})");

            if let Err(err) = std::fs::remove_dir_all(dir) {
                eprintln!("error cleaning up after {dir:?}: {err:?}");
            }
        }
    }
}

impl Cleanup {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, path: impl Into<PathBuf>, note: &'static str) {
        self.dirs.push((path.into(), note));
    }

    /// Create a temporary directory that is removed on cleanup.
    pub fn tmp_dir(&mut self, note: &'static str) -> Result<PathBuf> {
        let dir = make_tmp_dir("/tmp/", None)?;
        self.push(&dir, note);
        Ok(dir)
    }
}

/// A test image in a temporary directory, with another one to extract it to.
pub struct TestImage {
    path: PathBuf,
    pub extract_dir: PathBuf,
    _cleanup: Cleanup,
}

impl TestImage {
    pub fn new(layers: Vec<Vec<LayerEntry>>) -> Result<Self> {
        let mut cleanup = Cleanup::default();
        let path = cleanup.tmp_dir("image dir")?.join("oci_image.tar");
        let extract_dir = cleanup.tmp_dir("extract dir")?;

        layers
            .into_iter()
            .fold(OciTarImageBuilder::default(), OciTarImageBuilder::layer)
            .build(&path)?;

        Ok(Self {
            path,
            extract_dir,
            _cleanup: cleanup,
        })
    }

    pub fn extract(&self) -> Result<Option<Config>, ProxmoxOciError> {
        parse_and_extract_image(&self.path, &self.extract_dir, Some(&Arch::Amd64))
    }
}

/// Extract an image whose layers try to modify a directory outside of the extraction target, and
/// check that this fails with an illegal path error and leaves that directory untouched.
///
/// `layers` gets the path of that directory, which contains a `foreign.txt` file.
pub fn assert_breakout_rejected(
    layers: impl FnOnce(&Path) -> Vec<Vec<LayerEntry>>,
) -> Result<TestImage> {
    let mut cleanup = Cleanup::default();
    let foreign_dir = cleanup.tmp_dir("foreign dir")?;
    std::fs::write(foreign_dir.join("foreign.txt"), "Hello\n")?;

    let image = TestImage::new(layers(&foreign_dir))?;
    let err = image
        .extract()
        .expect_err("extracting an image with illegal paths should fail");
    assert!(
        err.to_string().contains("illegal path"),
        "unexpected error: {err}"
    );
    assert_eq!(read_to_string(foreign_dir.join("foreign.txt"))?, "Hello\n");

    Ok(image)
}
