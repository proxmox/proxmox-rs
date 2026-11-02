#![allow(dead_code)]

use std::path::PathBuf;

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
