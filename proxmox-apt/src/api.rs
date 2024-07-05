// API function that work without feature "cache"
use std::path::Path;

use anyhow::{bail, Error};

use proxmox_apt_api_types::{
    APTChangeRepositoryOptions, APTGetChangelogOptions, APTRepositoriesResult, APTRepositoryFile,
    APTRepositoryHandle,
};
use proxmox_config_digest::ConfigDigest;

use crate::repositories::{APTRepositoryFileImpl, APTRepositoryImpl};

/// Retrieve the changelog of the specified package.
pub fn get_changelog(options: &APTGetChangelogOptions) -> Result<String, Error> {
    let mut command = std::process::Command::new("apt-get");
    command.arg("changelog");
    command.arg("-qq"); // don't display download progress
    if let Some(ver) = &options.version {
        command.arg(format!("{}={}", options.name, ver));
    } else {
        command.arg(&options.name);
    }
    let output = proxmox_sys::command::run_command(command, None)?;

    Ok(output)
}

/// Get APT repository information.
pub fn list_repositories(product: &str) -> Result<APTRepositoriesResult, Error> {
    let apt_lists_dir = Path::new("/var/lib/apt/lists");

    let (files, errors, digest) = crate::repositories::repositories()?;

    let suite = crate::repositories::get_current_release_codename()?;

    let infos = crate::repositories::check_repositories(&files, suite, apt_lists_dir);
    let standard_repos = crate::repositories::standard_repositories(&files, product, suite);

    Ok(APTRepositoriesResult {
        files,
        errors,
        digest,
        infos,
        standard_repos,
    })
}

/// Add the repository identified by the `handle`.
/// If the repository is already configured, it will be set to enabled.
///
/// The `digest` parameter asserts that the configuration has not been modified.
pub fn add_repository_handle(
    product: &str,
    handle: APTRepositoryHandle,
    digest: Option<ConfigDigest>,
) -> Result<(), Error> {
    let (mut files, errors, current_digest) = crate::repositories::repositories()?;

    current_digest.detect_modification(digest.as_ref())?;

    let suite = crate::repositories::get_current_release_codename()?;

    // check if it's already configured first
    for file in files.iter_mut() {
        for repo in file.repositories.iter_mut() {
            if repo.is_referenced_repository(handle, "pbs", &suite.to_string()) {
                if repo.enabled {
                    return Ok(());
                }

                repo.set_enabled(true);
                file.write()?;

                return Ok(());
            }
        }
    }

    let (repo, path) = crate::repositories::get_standard_repository(handle, product, suite);

    if let Some(error) = errors.iter().find(|error| error.path == path) {
        bail!(
            "unable to parse existing file {} - {}",
            error.path,
            error.error,
        );
    }

    if let Some(file) = files
        .iter_mut()
        .find(|file| file.path.as_ref() == Some(&path))
    {
        file.repositories.push(repo);

        file.write()?;
    } else {
        let mut file = match APTRepositoryFile::new(&path)? {
            Some(file) => file,
            None => bail!("invalid path - {}", path),
        };

        file.repositories.push(repo);

        file.write()?;
    }

    Ok(())
}

/// Change the properties of the specified repository.
///
/// The `digest` parameter asserts that the configuration has not been modified.
pub fn change_repository(
    path: &str,
    index: usize,
    options: &APTChangeRepositoryOptions,
    digest: Option<ConfigDigest>,
) -> Result<(), Error> {
    let (mut files, errors, current_digest) = crate::repositories::repositories()?;

    current_digest.detect_modification(digest.as_ref())?;

    if let Some(error) = errors.iter().find(|error| error.path == path) {
        bail!("unable to parse file {} - {}", error.path, error.error);
    }

    if let Some(file) = files
        .iter_mut()
        .find(|file| file.path.as_deref() == Some(path))
    {
        if let Some(repo) = file.repositories.get_mut(index) {
            if let Some(enabled) = options.enabled {
                repo.set_enabled(enabled);
            }

            file.write()?;
        } else {
            bail!("invalid index - {}", index);
        }
    } else {
        bail!("invalid path - {}", path);
    }

    Ok(())
}
