use std::fs::write;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::Result;
use flate2::Compression;
use flate2::write::GzEncoder;
use nix::unistd::{getegid, geteuid};
use oci_spec::image::{
    Arch, Descriptor, DescriptorBuilder, ImageConfigurationBuilder, ImageIndexBuilder,
    ImageManifestBuilder, MediaType, OciLayoutBuilder, RootFsBuilder, Sha256Digest,
};
use sha2::{Digest, Sha256};

pub struct LayerEntry {
    path: PathBuf,
    kind: EntryKind,
}

enum EntryKind {
    File(Vec<u8>),
    Dir,
    Symlink(PathBuf),
}

impl LayerEntry {
    pub fn file(path: impl Into<PathBuf>, content: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.into(),
            kind: EntryKind::File(content.into()),
        }
    }

    pub fn dir(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            kind: EntryKind::Dir,
        }
    }

    pub fn symlink(path: impl Into<PathBuf>, target: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            kind: EntryKind::Symlink(target.into()),
        }
    }
}

#[derive(Default)]
pub struct OciTarImageBuilder {
    layers: Vec<Vec<LayerEntry>>,
}

impl OciTarImageBuilder {
    pub fn layer(mut self, entries: Vec<LayerEntry>) -> Self {
        self.layers.push(entries);
        self
    }

    pub fn build<P: AsRef<Path>>(self, oci_image_path: P) -> Result<()> {
        let mut tar = tar::Builder::new(Vec::new());

        let mut diff_ids = Vec::with_capacity(self.layers.len());
        let mut layer_descriptors = Vec::with_capacity(self.layers.len());

        let mut header = tar::Header::new_old();
        header.set_uid(geteuid().as_raw().into());
        header.set_gid(getegid().as_raw().into());

        for entries in self.layers {
            let mut layer_tar = tar::Builder::new(Vec::new());

            for entry in entries {
                let mut header = header.clone();
                let content = match entry.kind {
                    EntryKind::File(content) => {
                        header.set_entry_type(tar::EntryType::Regular);
                        content
                    }
                    EntryKind::Dir => {
                        header.set_entry_type(tar::EntryType::Directory);
                        header.set_mode(0o755);
                        vec![]
                    }
                    EntryKind::Symlink(target) => {
                        header.set_entry_type(tar::EntryType::Symlink);
                        header.set_link_name(target)?;
                        vec![]
                    }
                };
                header.set_size(content.len() as u64);
                header.set_cksum();
                let old = header.as_old_mut();
                let path = entry.path.as_os_str().as_encoded_bytes();
                if path.len() < old.name.len() {
                    // Skip path validity checks:
                    old.name[..path.len()].copy_from_slice(path);
                    header.set_cksum();
                    layer_tar.append(&header, &*content)?;
                } else {
                    layer_tar.append_data(&mut header, entry.path, &*content)?;
                }
            }

            let layer_bytes = layer_tar.into_inner()?;
            diff_ids.push(format!("sha256:{}", sha256(&layer_bytes)));

            let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
            gz.write_all(&layer_bytes)?;
            let layer_descriptor = add_blob(&mut tar, MediaType::ImageLayerGzip, &gz.finish()?)?;
            layer_descriptors.push(layer_descriptor);
        }

        let rootfs = RootFsBuilder::default()
            .typ("layers")
            .diff_ids(diff_ids)
            .build()?;
        let image_config = ImageConfigurationBuilder::default()
            .architecture(Arch::Amd64)
            .os("linux")
            .rootfs(rootfs)
            .build()?;
        let config_descriptor = add_blob(
            &mut tar,
            MediaType::ImageConfig,
            &image_config.to_string()?.into_bytes(),
        )?;

        let manifest = ImageManifestBuilder::default()
            .schema_version(2u32)
            .config(config_descriptor)
            .layers(layer_descriptors)
            .build()?;
        let manifest_descriptor = add_blob(
            &mut tar,
            MediaType::ImageManifest,
            &manifest.to_string()?.into_bytes(),
        )?;

        let index = ImageIndexBuilder::default()
            .schema_version(2u32)
            .manifests([manifest_descriptor])
            .build()?;
        let index_bytes = index.to_string()?.into_bytes();
        add_file(&mut tar, "index.json", &index_bytes)?;

        let layout = OciLayoutBuilder::default()
            .image_layout_version("1.0.0")
            .build()?;
        let layout_bytes = layout.to_string()?.into_bytes();
        add_file(&mut tar, "oci-layout", &layout_bytes)?;

        Ok(write(oci_image_path, tar.into_inner()?)?)
    }
}

fn sha256(data: &[u8]) -> Sha256Digest {
    Sha256Digest::from_str(&format!("{:x}", Sha256::digest(data))).expect("digest must be valid")
}

fn add_blob<W: Write>(
    tar: &mut tar::Builder<W>,
    media_type: MediaType,
    data: &[u8],
) -> oci_spec::Result<Descriptor> {
    let hash = sha256(data);
    add_file(tar, &format!("blobs/sha256/{hash}"), data)?;
    DescriptorBuilder::default()
        .media_type(media_type)
        .digest(hash)
        .size(data.len() as u64)
        .build()
}

fn add_file<W: Write>(tar: &mut tar::Builder<W>, path: &str, data: &[u8]) -> std::io::Result<()> {
    let mut header = tar::Header::new_old();
    header.set_size(data.len() as u64);
    header.set_cksum();
    tar.append_data(&mut header, path, data)
}
