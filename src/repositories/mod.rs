use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{bail, Error};

mod repository;
pub use repository::{
    APTRepository, APTRepositoryFileType, APTRepositoryOption, APTRepositoryPackageType,
};

mod file;
pub use file::{APTRepositoryFile, APTRepositoryFileError};

const APT_SOURCES_LIST_FILENAME: &str = "/etc/apt/sources.list";
const APT_SOURCES_LIST_DIRECTORY: &str = "/etc/apt/sources.list.d/";

/// Calculates a common digest for successfully parsed repository files.
///
/// The digest is invariant with respect to file order.
///
/// Files without a digest are ignored.
fn common_digest(files: &[APTRepositoryFile]) -> [u8; 32] {
    let mut digests = BTreeMap::new();

    for file in files.iter() {
        digests.insert(file.path.clone(), &file.digest);
    }

    let mut common_raw = Vec::<u8>::with_capacity(digests.len() * 32);
    for digest in digests.values() {
        match digest {
            Some(digest) => common_raw.extend_from_slice(&digest[..]),
            None => (),
        }
    }

    openssl::sha::sha256(&common_raw[..])
}

/// Returns all APT repositories configured in `/etc/apt/sources.list` and
/// in `/etc/apt/sources.list.d` including disabled repositories.
///
/// Returns the succesfully parsed files, a list of errors for files that could
/// not be read or parsed and a common digest for the succesfully parsed files.
///
/// The digest is guaranteed to be set for each successfully parsed file.
pub fn repositories() -> Result<
    (
        Vec<APTRepositoryFile>,
        Vec<APTRepositoryFileError>,
        [u8; 32],
    ),
    Error,
> {
    let to_result = |files: Vec<APTRepositoryFile>, errors: Vec<APTRepositoryFileError>| {
        let common_digest = common_digest(&files);

        (files, errors, common_digest)
    };

    let mut files = vec![];
    let mut errors = vec![];

    let sources_list_path = PathBuf::from(APT_SOURCES_LIST_FILENAME);

    let sources_list_d_path = PathBuf::from(APT_SOURCES_LIST_DIRECTORY);

    match APTRepositoryFile::new(sources_list_path) {
        Ok(Some(mut file)) => match file.parse() {
            Ok(()) => files.push(file),
            Err(err) => errors.push(err),
        },
        _ => bail!("internal error with '{}'", APT_SOURCES_LIST_FILENAME),
    }

    if !sources_list_d_path.exists() {
        return Ok(to_result(files, errors));
    }

    if !sources_list_d_path.is_dir() {
        errors.push(APTRepositoryFileError {
            path: APT_SOURCES_LIST_DIRECTORY.to_string(),
            error: "not a directory!".to_string(),
        });
        return Ok(to_result(files, errors));
    }

    for entry in std::fs::read_dir(sources_list_d_path)? {
        let path = entry?.path();

        match APTRepositoryFile::new(path) {
            Ok(Some(mut file)) => match file.parse() {
                Ok(()) => {
                    if file.digest.is_none() {
                        bail!("internal error - digest not set");
                    }
                    files.push(file);
                }
                Err(err) => errors.push(err),
            },
            Ok(None) => (),
            Err(err) => errors.push(err),
        }
    }

    Ok(to_result(files, errors))
}
