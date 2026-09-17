#![allow(dead_code)]

use std::path::PathBuf;

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
}
