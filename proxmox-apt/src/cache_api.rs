// API function that need feature "cache"
use std::path::Path;

use anyhow::{bail, format_err, Error};
use std::os::unix::prelude::OsStrExt;

use proxmox_apt_api_types::{APTUpdateInfo, APTUpdateOptions};

/// List available APT updates
///
/// Automatically updates an expired package cache.
pub fn list_available_apt_update<P: AsRef<Path>>(
    apt_state_file: P,
) -> Result<Vec<APTUpdateInfo>, Error> {
    let apt_state_file = apt_state_file.as_ref();
    if let Ok(false) = crate::cache::pkg_cache_expired(apt_state_file) {
        if let Ok(Some(cache)) = crate::cache::read_pkg_state(apt_state_file) {
            return Ok(cache.package_status);
        }
    }

    let cache = crate::cache::update_cache(apt_state_file)?;

    Ok(cache.package_status)
}

/// Update the APT database
///
/// You should update the APT proxy configuration before running this.
pub fn update_database<P: AsRef<Path>>(
    apt_state_file: P,
    options: &APTUpdateOptions,
    send_updates_available: impl Fn(&[&APTUpdateInfo]) -> Result<(), Error>,
) -> Result<(), Error> {
    let apt_state_file = apt_state_file.as_ref();

    let quiet = options.quiet.unwrap_or(false);
    let notify = options.notify.unwrap_or(false);

    if !quiet {
        log::info!("starting apt-get update")
    }

    let mut command = std::process::Command::new("apt-get");
    command.arg("update");

    // apt "errors" quite easily, and run_command is a bit rigid, so handle this inline for now.
    let output = command
        .output()
        .map_err(|err| format_err!("failed to execute {:?} - {}", command, err))?;

    if !quiet {
        log::info!("{}", String::from_utf8(output.stdout)?);
    }

    // TODO: improve run_command to allow outputting both, stderr and stdout
    if !output.status.success() {
        if output.status.code().is_some() {
            let msg = String::from_utf8(output.stderr)
                .map(|m| {
                    if m.is_empty() {
                        String::from("no error message")
                    } else {
                        m
                    }
                })
                .unwrap_or_else(|_| String::from("non utf8 error message (suppressed)"));
            log::warn!("{msg}");
        } else {
            bail!("terminated by signal");
        }
    }

    let mut cache = crate::cache::update_cache(apt_state_file)?;

    if notify {
        let mut notified = cache.notified.unwrap_or_default();
        let mut to_notify: Vec<&APTUpdateInfo> = Vec::new();

        for pkg in &cache.package_status {
            match notified.insert(pkg.package.to_owned(), pkg.version.to_owned()) {
                Some(notified_version) => {
                    if notified_version != pkg.version {
                        to_notify.push(pkg);
                    }
                }
                None => to_notify.push(pkg),
            }
        }
        if !to_notify.is_empty() {
            to_notify.sort_unstable_by_key(|k| &k.package);
            send_updates_available(&to_notify)?;
        }
        cache.notified = Some(notified);
        crate::cache::write_pkg_cache(apt_state_file, &cache)?;
    }

    Ok(())
}

/// Get package information for a list of important product packages.
///
/// We first list the product virtual package (i.e. `proxmox-backup`), with extra
/// information about the running kernel.
///
/// Next is the api_server_package, with extra information abnout the running api
/// server version.
///
/// The list of installed kernel packages follows.
///
/// We the add an entry for all packages in package_list, even if they are
/// not installed.
pub fn get_package_versions(
    product_virtual_package: &str,
    api_server_package: &str,
    running_api_server_version: &str,
    package_list: &[&str],
) -> Result<Vec<APTUpdateInfo>, Error> {
    fn unknown_package(package: String, extra_info: Option<String>) -> APTUpdateInfo {
        APTUpdateInfo {
            package,
            title: "unknown".into(),
            arch: "unknown".into(),
            description: "unknown".into(),
            version: "unknown".into(),
            old_version: None, // it's unknown if there is any old version, but None isn't wrong.
            origin: "unknown".into(),
            priority: "unknown".into(),
            section: "unknown".into(),
            extra_info,
        }
    }

    let mut packages: Vec<APTUpdateInfo> = Vec::new();

    let is_kernel =
        |name: &str| name.starts_with("pve-kernel-") || name.starts_with("proxmox-kernel");

    let installed_packages = crate::cache::list_installed_apt_packages(
        |filter| {
            filter.installed_version == Some(filter.active_version)
                && (is_kernel(filter.package)
                    || (filter.package == product_virtual_package)
                    || (filter.package == api_server_package)
                    || package_list.contains(&filter.package))
        },
        None,
    );

    let running_kernel = format!(
        "running kernel: {}",
        std::str::from_utf8(nix::sys::utsname::uname()?.release().as_bytes())?.to_owned()
    );

    if let Some(product_virtual_package_info) = installed_packages
        .iter()
        .find(|pkg| pkg.package == product_virtual_package)
    {
        let mut product_virtual_package_info = product_virtual_package_info.clone();
        product_virtual_package_info.extra_info = Some(running_kernel);
        packages.push(product_virtual_package_info);
    } else {
        packages.push(unknown_package(
            product_virtual_package.into(),
            Some(running_kernel),
        ));
    }

    if let Some(api_server_package_info) = installed_packages
        .iter()
        .find(|pkg| pkg.package == api_server_package)
    {
        let mut api_server_package_info = api_server_package_info.clone();
        api_server_package_info.extra_info = Some(running_api_server_version.into());
        packages.push(api_server_package_info);
    } else {
        packages.push(unknown_package(
            api_server_package.into(),
            Some(running_api_server_version.into()),
        ));
    }

    let mut kernel_pkgs: Vec<APTUpdateInfo> = installed_packages
        .iter()
        .filter(|pkg| is_kernel(&pkg.package))
        .cloned()
        .collect();

    crate::cache::sort_package_list(&mut kernel_pkgs);

    packages.append(&mut kernel_pkgs);

    // add entry for all packages we're interested in, even if not installed
    for pkg in package_list.iter() {
        if *pkg == product_virtual_package || *pkg == api_server_package {
            continue;
        }
        match installed_packages.iter().find(|item| &item.package == pkg) {
            Some(apt_pkg) => packages.push(apt_pkg.to_owned()),
            None => packages.push(unknown_package(pkg.to_string(), None)),
        }
    }

    Ok(packages)
}
