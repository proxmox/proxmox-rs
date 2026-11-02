use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::{File, read_dir, remove_dir_all, remove_file};
use std::io::{self, ErrorKind, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use flate2::read::GzDecoder;
use oci_spec::OciSpecError;
pub use oci_spec::image::{Arch, Config};
use oci_spec::image::{ImageConfiguration, ImageManifest, MediaType};
use sha2::digest::generic_array::GenericArray;
use sha2::{Digest, Sha256};
use tar::{Archive, EntryType};
use thiserror::Error;

mod oci_tar_image;
use oci_tar_image::{OciTarImage, OciTarImageBlob};

const WHITEOUT_PREFIX: &str = ".wh.";
const OPAQUE_WHITEOUT_NAME: &str = ".wh..wh..opq";

fn compute_digest<R: Read, H: Digest>(
    mut reader: R,
    mut hasher: H,
) -> io::Result<GenericArray<u8, H::OutputSize>> {
    let mut buf = proxmox_io::boxed::zeroed(32768);

    loop {
        let bytes_read = reader.read(&mut buf)?;
        if bytes_read == 0 {
            break Ok(hasher.finalize());
        }

        hasher.update(&buf[..bytes_read]);
    }
}

fn compute_sha256<R: Read>(reader: R) -> io::Result<oci_spec::image::Sha256Digest> {
    let digest = compute_digest(reader, Sha256::new())?;
    Ok(oci_spec::image::Sha256Digest::from_str(&format!("{digest:x}")).expect("valid digest"))
}

/// Build a mapping from uncompressed layer digests (as found in the image config's `rootfs.diff_ids`)
/// to their corresponding compressed-layer digests (i.e. the filenames under `blobs/<algorithm>/<digest>`)
fn build_layer_map<R: Read + Seek>(
    mut oci_tar_image: OciTarImage<R>,
    image_manifest: &ImageManifest,
) -> Result<
    (
        OciTarImage<R>,
        HashMap<oci_spec::image::Digest, oci_spec::image::Descriptor>,
    ),
    ExtractError,
> {
    let mut layer_mapping = HashMap::new();

    for layer in image_manifest.layers() {
        let digest = match layer.media_type() {
            MediaType::ImageLayer | MediaType::ImageLayerNonDistributable => layer.digest().clone(),
            MediaType::ImageLayerGzip | MediaType::ImageLayerNonDistributableGzip => {
                let mut compressed_blob = oci_tar_image
                    .open_blob(layer.digest())
                    .ok_or(ExtractError::MissingLayerFile(layer.digest().clone()))?;
                let decoder = GzDecoder::new(&mut compressed_blob);
                let hash = compute_sha256(decoder)?.into();
                oci_tar_image = compressed_blob.into_oci_tar_image();
                hash
            }
            MediaType::ImageLayerZstd | MediaType::ImageLayerNonDistributableZstd => {
                let mut compressed_blob = oci_tar_image
                    .open_blob(layer.digest())
                    .ok_or(ExtractError::MissingLayerFile(layer.digest().clone()))?;
                let decoder = zstd::Decoder::new(&mut compressed_blob)?;
                let hash = compute_sha256(decoder)?.into();
                oci_tar_image = compressed_blob.into_oci_tar_image();
                hash
            }
            // Skip any other non-ImageLayer related media types.
            // Match explicitly to avoid missing new image layer types when oci-spec updates.
            MediaType::Descriptor
            | MediaType::LayoutHeader
            | MediaType::ImageManifest
            | MediaType::ImageIndex
            | MediaType::ImageConfig
            | MediaType::ArtifactManifest
            | MediaType::EmptyJSON
            | MediaType::Other(_) => continue,
        };

        layer_mapping.insert(digest, layer.clone());
    }

    Ok((oci_tar_image, layer_mapping))
}

#[derive(Debug, Error)]
pub enum ProxmoxOciError {
    #[error("Error while parsing OCI image: {0}")]
    ParseError(#[from] ParseError),
    #[error("Error while extracting OCI image: {0}")]
    ExtractError(#[from] ExtractError),
}

/// Extract the rootfs of an OCI image tar and return its execution parameters.
///
/// # Arguments
///
/// * `oci_tar_path` - Path to the OCI image tar archive
/// * `rootfs_path` - Destination path where the rootfs will be extracted to
/// * `arch` - Optional CPU architecture used to pick the first matching manifest from a multi-arch
///   image index. If `None`, the first manifest will be used.
///
/// # Returns
///
/// On success, this returns the optional container execution parameters
/// if present in the image configuration.
pub fn parse_and_extract_image<P: AsRef<Path>>(
    oci_tar_path: P,
    rootfs_path: P,
    arch: Option<&Arch>,
) -> Result<Option<Config>, ProxmoxOciError> {
    let (oci_tar_image, image_manifest, image_config) = parse_image(oci_tar_path, arch)?;

    extract_image_rootfs(oci_tar_image, &image_manifest, &image_config, rootfs_path)?;

    Ok(image_config.config().clone())
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("OCI spec error: {0}")]
    OciSpec(#[from] OciSpecError),
    #[error("Wrong media type")]
    WrongMediaType,
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Unsupported CPU architecture")]
    UnsupportedArchitecture,
    #[error("Missing image config")]
    MissingImageConfig,
}

fn parse_image<P: AsRef<Path>>(
    oci_tar_path: P,
    arch: Option<&Arch>,
) -> Result<(OciTarImage<File>, ImageManifest, ImageConfiguration), ParseError> {
    let oci_tar_file = File::open(oci_tar_path)?;
    let mut oci_tar_image = OciTarImage::new(oci_tar_file)?;

    let image_manifest = oci_tar_image
        .image_manifest(arch)
        .ok_or(ParseError::UnsupportedArchitecture)??;

    let image_config_descriptor = image_manifest.config();

    if image_config_descriptor.media_type() != &MediaType::ImageConfig {
        return Err(ParseError::WrongMediaType);
    }

    let mut image_config_file = oci_tar_image
        .open_blob(image_config_descriptor.digest())
        .ok_or(ParseError::MissingImageConfig)?;
    let image_config = ImageConfiguration::from_reader(&mut image_config_file)?;

    Ok((
        image_config_file.into_oci_tar_image(),
        image_manifest,
        image_config,
    ))
}

#[derive(Debug, Error)]
pub enum ExtractError {
    #[error("Incorrectly formatted digest: \"{0}\"")]
    InvalidDigest(String),
    #[error("Unknown layer digest {0} found in rootfs.diff_ids")]
    UnknownLayerDigest(oci_spec::image::Digest),
    #[error("Layer file {0} mentioned in image manifest is missing")]
    MissingLayerFile(oci_spec::image::Digest),
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Layer has wrong media type: {0}")]
    WrongMediaType(String),
}

fn extract_image_rootfs<R: Read + Seek, P: AsRef<Path>>(
    oci_tar_image: OciTarImage<R>,
    image_manifest: &ImageManifest,
    image_config: &ImageConfiguration,
    target_path: P,
) -> Result<(), ExtractError> {
    let (mut oci_tar_image, layer_map) = build_layer_map(oci_tar_image, image_manifest)?;
    let target_path = target_path.as_ref().canonicalize()?;

    for layer in image_config.rootfs().diff_ids() {
        let layer_digest = oci_spec::image::Digest::from_str(layer)
            .map_err(|_| ExtractError::InvalidDigest(layer.to_string()))?;
        let layer_descriptor = layer_map
            .get(&layer_digest)
            .ok_or(ExtractError::UnknownLayerDigest(layer_digest.clone()))?;
        let mut layer_file = oci_tar_image
            .open_blob(layer_descriptor.digest())
            .ok_or(ExtractError::MissingLayerFile(layer_digest))?;

        type DecodeFn<T> = Box<dyn for<'a> Fn(&'a mut T) -> io::Result<Box<dyn Read + 'a>>>;
        let decode_fn: DecodeFn<OciTarImageBlob<R>> = match layer_descriptor.media_type() {
            MediaType::ImageLayer | MediaType::ImageLayerNonDistributable => {
                Box::new(|file| Ok(Box::new(file)))
            }
            MediaType::ImageLayerGzip | MediaType::ImageLayerNonDistributableGzip => {
                Box::new(|file| Ok(Box::new(GzDecoder::new(file))))
            }
            MediaType::ImageLayerZstd | MediaType::ImageLayerNonDistributableZstd => {
                Box::new(|file| Ok(Box::new(zstd::Decoder::new(file)?)))
            }
            // Error on any other non-ImageLayer related media types.
            // Match explicitly to avoid missing new image layer types when oci-spec updates.
            media_type @ (MediaType::Descriptor
            | MediaType::LayoutHeader
            | MediaType::ImageManifest
            | MediaType::ImageIndex
            | MediaType::ImageConfig
            | MediaType::ArtifactManifest
            | MediaType::EmptyJSON
            | MediaType::Other(_)) => {
                return Err(ExtractError::WrongMediaType(media_type.to_string()));
            }
        };

        remove_replaced_paths(&mut decode_fn(&mut layer_file)?, &target_path)?;
        layer_file.seek(SeekFrom::Start(0))?;
        extract_archive(&mut decode_fn(&mut layer_file)?, &target_path)?;

        oci_tar_image = layer_file.into_oci_tar_image();
    }

    Ok(())
}

/// Validate a path previously received from a `tar::Entry`.
///
/// Tar files may contain arbitrary paths, so reject absolute paths and paths containing parent
/// directory references, as joining them onto the target directory would lead outside of it.
pub(crate) fn validate_path(path: &Path) -> io::Result<()> {
    use std::path::Component;

    if !path
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(io::Error::other(format!(
            "illegal path {path:?} in layer - absolute paths and '..' components are not allowed"
        )));
    }

    Ok(())
}

/// Join the `path` of a layer entry onto the resolved `target_path`, resolving its parent only.
///
/// Fails if the parent leads outside of the target through a symlink, like the tar crate checks
/// before unpacking, assuming that nothing else modifies the target meanwhile. Returns `None` if
/// the parent does not exist or `path` has no last component.
fn resolve_in_target(target_path: &Path, path: &Path) -> io::Result<Option<PathBuf>> {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Ok(None);
    };
    Ok(resolve_dir_in_target(target_path, parent)?.map(|dir| dir.join(name)))
}

/// Resolve the directory `dir` of a layer entry within the resolved `target_path`, see
/// [`resolve_in_target`].
fn resolve_dir_in_target(target_path: &Path, dir: &Path) -> io::Result<Option<PathBuf>> {
    let resolved = match target_path.join(dir).canonicalize() {
        Ok(resolved) => resolved,
        Err(err) if matches!(err.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
            return Ok(None);
        }
        Err(err) => return Err(err),
    };

    if !resolved.starts_with(target_path) {
        return Err(io::Error::other(format!(
            "illegal path {dir:?} in layer - a symlink leads outside of the target directory"
        )));
    }

    Ok(Some(resolved))
}

/// Remove what a layer replaces with directories or whites out from previous layers.
fn remove_replaced_paths<R: Read>(reader: &mut R, target_path: &Path) -> io::Result<()> {
    let mut archive = Archive::new(reader);

    let mut directories = Vec::new();
    let mut whiteouts = Vec::new();
    for entry in archive.entries()? {
        let entry = entry?;
        let path = entry.path()?;
        validate_path(&path)?;

        match entry.header().entry_type() {
            EntryType::Directory => directories.push(path.into_owned()),
            EntryType::Regular if is_whiteout(&path) => whiteouts.push(path.into_owned()),
            _ => {}
        }
    }

    // Parents first and before whiteouts, so nothing resolves through a symlink being replaced.
    directories.sort();
    for dir in directories {
        if let Some(dir_path_abs) = resolve_in_target(target_path, &dir)?
            && dir_path_abs.symlink_metadata().is_ok_and(|m| !m.is_dir())
        {
            remove_file(dir_path_abs)?;
        }
    }

    for whiteout in whiteouts {
        apply_whiteout(target_path, &whiteout)?;
    }

    Ok(())
}

fn is_whiteout(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.starts_with(WHITEOUT_PREFIX))
}

/// Apply the whiteout file at `path` on previous layers.
fn apply_whiteout(target_path: &Path, path: &Path) -> io::Result<()> {
    let Some(filename) = path.file_name().and_then(OsStr::to_str) else {
        return Ok(());
    };

    if filename == OPAQUE_WHITEOUT_NAME {
        if let Some(parent) = path.parent()
            && let Some(whiteout_abs_path) = resolve_dir_in_target(target_path, parent)?
        {
            for direntry in read_dir(whiteout_abs_path)? {
                remove_path(&direntry?.path())?;
            }
        }
    } else if let Some(filename_stripped) = filename.strip_prefix(WHITEOUT_PREFIX) {
        let whiteout_path = match filename_stripped {
            "." => match path.parent() {
                Some(p) if p.parent().is_some() => p,
                _ => return Ok(()), // Prevent whiteout of root directory
            },
            ".." => return Ok(()), // Prevent whiteout of grandparent directory
            fname => &path.with_file_name(fname),
        };
        if let Some(whiteout_abs_path) = resolve_in_target(target_path, whiteout_path)? {
            remove_path(&whiteout_abs_path)?;
        }
    }

    Ok(())
}

fn extract_archive<R: Read>(reader: &mut R, target_path: &Path) -> io::Result<()> {
    let mut archive = Archive::new(reader);
    archive.set_preserve_ownerships(true);
    archive.set_preserve_permissions(true);
    archive.set_unpack_xattrs(true);

    // Delay directory entries until the end (they will be created if needed by descendants),
    // to ensure that directory permissions do not interfere with descendant extraction.
    let mut directories = Vec::new();
    for entry in archive.entries()? {
        let mut file = entry?;

        let filepath = file.path()?;
        validate_path(&filepath)?;

        if file.header().entry_type() == EntryType::Directory {
            directories.push(file);
            continue;
        } else if file.header().entry_type() == EntryType::Regular && is_whiteout(&filepath) {
            continue;
        }

        // If a file or directory already exists at this path, remove it first.
        if let Some(file_path_abs) = resolve_in_target(target_path, &filepath)? {
            remove_path(&file_path_abs)?;
        }

        file.unpack_in(target_path)?;
    }

    // Apply the directories in reverse topological order,
    // to avoid failure on restrictive parent directory permissions.
    directories.sort_by(|a, b| b.path_bytes().cmp(&a.path_bytes()));
    for mut dir in directories {
        // Skip a directory replaced by a later entry, also to not apply it through a symlink.
        if let Some(dir_path_abs) = resolve_in_target(target_path, &dir.path()?)?
            && dir_path_abs.symlink_metadata().is_ok_and(|m| !m.is_dir())
        {
            continue;
        }

        dir.unpack_in(target_path)?;
    }

    Ok(())
}

/// Remove whatever exists at `path`, without following a symlink there.
fn remove_path(path: &Path) -> io::Result<()> {
    match path.symlink_metadata() {
        Ok(metadata) if metadata.is_dir() => remove_dir_all(path),
        Ok(_) => remove_file(path),
        Err(err) if matches!(err.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => Ok(()),
        Err(err) => Err(err),
    }
}
