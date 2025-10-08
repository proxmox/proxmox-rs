use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::ops::Range;
use std::path::{Path, PathBuf};

use oci_spec::image::{Arch, Digest, ImageIndex, ImageManifest, MediaType};
use oci_spec::OciSpecError;
use tar::Archive;

use proxmox_io::RangeReader;

#[derive(Clone)]
struct TarEntry {
    range: Range<u64>,
}

impl TarEntry {
    fn new(range: Range<u64>) -> Self {
        Self { range }
    }
}

pub struct OciTarImage<R: Read + Seek> {
    reader: R,
    entries: HashMap<PathBuf, TarEntry>,
    image_index: ImageIndex,
}

impl<R: Read + Seek> OciTarImage<R> {
    pub fn new(reader: R) -> oci_spec::Result<Self> {
        let mut archive = Archive::new(reader);
        let entries = archive.entries_with_seek()?;
        let mut entries_index = HashMap::new();
        let mut image_index = None;

        for entry in entries {
            let mut entry = entry?;
            let offset = entry.raw_file_position();
            let size = entry.size();
            let path = entry.path()?.into_owned();

            if path.as_path() == Path::new("index.json") {
                image_index = Some(ImageIndex::from_reader(&mut entry)?);
            }

            let tar_entry = TarEntry::new(offset..(offset + size));
            entries_index.insert(path, tar_entry);
        }

        if let Some(image_index) = image_index {
            Ok(Self {
                reader: archive.into_inner(),
                entries: entries_index,
                image_index,
            })
        } else {
            Err(OciSpecError::Other("Missing index.json file".into()))
        }
    }

    pub fn image_index(&self) -> &ImageIndex {
        &self.image_index
    }

    fn get_blob_entry(&self, digest: &Digest) -> Option<TarEntry> {
        let path = get_blob_path(digest);
        self.entries.get(&path).cloned()
    }

    pub fn open_blob(self, digest: &Digest) -> Option<OciTarImageBlob<R>> {
        if let Some(entry) = self.get_blob_entry(digest) {
            Some(OciTarImageBlob::new(self, entry.range))
        } else {
            None
        }
    }

    pub fn image_manifest(
        &mut self,
        architecture: Option<&Arch>,
    ) -> Option<oci_spec::Result<ImageManifest>> {
        let digest = match self.image_index.manifests().iter().find(|d| {
            d.media_type() == &MediaType::ImageManifest
                && architecture
                    .is_none_or(|a| d.platform().as_ref().is_none_or(|p| p.architecture() == a))
        }) {
            Some(descriptor) => descriptor.digest(),
            None => return None,
        };

        if let Some(entry) = self.get_blob_entry(digest) {
            let mut range_reader = RangeReader::new(&mut self.reader, entry.range);
            Some(ImageManifest::from_reader(&mut range_reader))
        } else {
            Some(Err(OciSpecError::Other(format!(
                "Image manifest with digest {digest} mentioned in image index is missing"
            ))))
        }
    }
}

fn get_blob_path(digest: &Digest) -> PathBuf {
    let algorithm = digest.algorithm();
    let digest = digest.digest();
    format!("blobs/{algorithm}/{digest}").into()
}

pub struct OciTarImageBlob<R: Read + Seek> {
    range_reader: RangeReader<R>,
    entries: HashMap<PathBuf, TarEntry>,
    image_index: ImageIndex,
}

impl<R: Read + Seek> OciTarImageBlob<R> {
    fn new(archive: OciTarImage<R>, range: Range<u64>) -> Self {
        let range_reader = RangeReader::new(archive.reader, range);

        Self {
            range_reader,
            entries: archive.entries,
            image_index: archive.image_index,
        }
    }

    pub fn into_oci_tar_image(self) -> OciTarImage<R> {
        OciTarImage {
            reader: self.range_reader.into_inner(),
            entries: self.entries,
            image_index: self.image_index,
        }
    }
}

impl<R: Read + Seek> Read for OciTarImageBlob<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.range_reader.read(buf)
    }
}

impl<R: Read + Seek> Seek for OciTarImageBlob<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.range_reader.seek(pos)
    }
}
